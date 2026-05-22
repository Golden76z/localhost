use crate::config::Config;
use crate::events::{emit, EventSender, ServerEvent};
use crate::http::Parser;
use crate::net::{timed_out, EpollLoop, IoKind, SocketFd};
use crate::server::handler::{parse_error_response, RequestHandler};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Instant;

enum ConnPhase {
    Reading {
        parser: Parser,
        last: Instant,
    },
    Writing {
        buf: Vec<u8>,
        offset: usize,
        last: Instant,
        sse_after: bool,
    },
    Sse {
        pending: Vec<u8>,
        offset: usize,
        last: Instant,
    },
}

struct Connection {
    listener_port: u16,
    phase: ConnPhase,
    read_buf: [u8; 8192],
}

pub struct ServerEngine;

impl ServerEngine {
    pub fn run(config: Config) -> Result<(), String> {
        Self::run_with_events(config, None)
    }

    pub fn run_with_events(config: Config, events: Option<EventSender>) -> Result<(), String> {
        for e in &config.errors {
            eprintln!("config warning: {e}");
        }

        let mut poll = EpollLoop::new()?;
        let mut listeners: HashMap<SocketFd, u16> = HashMap::new();
        let mut connections: HashMap<SocketFd, Connection> = HashMap::new();
        let mut handler = RequestHandler::new(events.clone());

        for server in &config.servers {
            let ip: Ipv4Addr = server
                .listen_host
                .parse()
                .unwrap_or(Ipv4Addr::UNSPECIFIED);
            let addr = SocketAddrV4::new(ip, server.listen_port);
            let fd = poll.bind(&addr)?;
            listeners.insert(fd, server.listen_port);
            emit(
                &events,
                ServerEvent::Listener {
                    addr: addr.to_string(),
                },
            );
            eprintln!("listening on {addr}");
        }

        loop {
            let poll_events = poll.wait(1000)?;
            let now = Instant::now();

            let mut to_remove = Vec::new();
            for ev in &poll_events {
                if ev.hangup {
                    to_remove.push(ev.fd);
                    continue;
                }

                match ev.kind {
                    IoKind::Listener => {
                        if ev.readable {
                            match poll.accept(ev.fd) {
                                Ok(Some((client, _))) => {
                                    let port = listeners[&ev.fd];
                                    let max = config
                                        .resolve_server("0.0.0.0", port, None)
                                        .client_max_body_size;
                                    connections.insert(
                                        client,
                                        Connection {
                                            listener_port: port,
                                            phase: ConnPhase::Reading {
                                                parser: Parser::new(max),
                                                last: now,
                                            },
                                            read_buf: [0u8; 8192],
                                        },
                                    );
                                }
                                Ok(None) => {}
                                Err(_) => to_remove.push(ev.fd),
                            }
                        }
                    }
                    IoKind::Client => {
                        if let Some(conn) = connections.get_mut(&ev.fd) {
                            if handle_client_io(
                                &mut poll,
                                ev.fd,
                                conn,
                                &config,
                                &mut handler,
                                ev,
                            ) {
                                to_remove.push(ev.fd);
                            }
                        }
                    }
                }
            }

            if let Some(data) = handler.take_pending_broadcast() {
                push_sse(&mut handler, &mut connections, &mut poll, &data);
            }

            flush_sse_writes(&mut poll, &mut connections, &mut to_remove);

            for fd in to_remove {
                handler.unregister_sse(fd);
                poll.remove(fd);
                connections.remove(&fd);
            }

            let mut timed = Vec::new();
            for (&fd, conn) in &connections {
                let last = match conn.phase {
                    ConnPhase::Reading { last, .. } => last,
                    ConnPhase::Writing { last, .. } => last,
                    ConnPhase::Sse { last, .. } => last,
                };
                if timed_out(last) {
                    timed.push(fd);
                }
            }
            for fd in timed {
                handler.unregister_sse(fd);
                poll.remove(fd);
                connections.remove(&fd);
            }
        }
    }
}

fn push_sse(
    handler: &mut RequestHandler,
    connections: &mut HashMap<SocketFd, Connection>,
    poll: &mut EpollLoop,
    data: &str,
) {
    let bytes = data.as_bytes();
    for &fd in handler.sse_clients.clone().iter() {
        if let Some(conn) = connections.get_mut(&fd) {
            if let ConnPhase::Sse { pending, last, .. } = &mut conn.phase
            {
                pending.extend_from_slice(bytes);
                *last = Instant::now();
                let _ = poll.mod_interest(fd, true);
            }
        }
    }
}

fn flush_sse_writes(
    poll: &mut EpollLoop,
    connections: &mut HashMap<SocketFd, Connection>,
    to_remove: &mut Vec<SocketFd>,
) {
    let fds: Vec<SocketFd> = connections.keys().copied().collect();
    for fd in fds {
        let Some(conn) = connections.get_mut(&fd) else {
            continue;
        };
        let ConnPhase::Sse {
            pending,
            offset,
            last,
        } = &mut conn.phase
        else {
            continue;
        };
        if *offset >= pending.len() {
            continue;
        }
        match poll.write(fd, &pending[*offset..]) {
            Ok(0) => {}
            Ok(n) => {
                *offset += n;
                *last = Instant::now();
                if *offset >= pending.len() {
                    pending.clear();
                    *offset = 0;
                    let _ = poll.mod_interest(fd, false);
                }
            }
            Err(_) => to_remove.push(fd),
        }
    }
}

fn handle_client_io(
    poll: &mut EpollLoop,
    fd: SocketFd,
    conn: &mut Connection,
    config: &Config,
    handler: &mut RequestHandler,
    ev: &crate::net::IoEvent,
) -> bool {
    match &mut conn.phase {
        ConnPhase::Reading { parser, last } => {
            if !ev.readable {
                return false;
            }
            let n = match poll.read(fd, &mut conn.read_buf) {
                Ok(Some(0)) | Ok(None) => return true,
                Ok(Some(n)) if n == 0 => return false,
                Ok(Some(n)) => n,
                Err(_) => return true,
            };
            *last = Instant::now();

            let server = config.resolve_server("0.0.0.0", conn.listener_port, None);
            match parser.feed(&conn.read_buf[..n]) {
                Ok(Some(req)) => {
                    let resp = handler.handle(config, req, conn.listener_port);
                    let sse_after = resp.sse_keep_alive;
                    let bytes = resp.to_bytes();
                    let _ = poll.mod_interest(fd, true);
                    conn.phase = ConnPhase::Writing {
                        buf: bytes,
                        offset: 0,
                        last: Instant::now(),
                        sse_after,
                    };
                }
                Ok(None) => {}
                Err(e) => {
                    let resp = parse_error_response(server, e);
                    let bytes = resp.to_bytes();
                    let _ = poll.mod_interest(fd, true);
                    conn.phase = ConnPhase::Writing {
                        buf: bytes,
                        offset: 0,
                        last: Instant::now(),
                        sse_after: false,
                    };
                }
            }
            false
        }
        ConnPhase::Writing {
            buf,
            offset,
            last,
            sse_after,
        } => {
            if !ev.writable {
                return false;
            }
            if *offset >= buf.len() {
                if *sse_after {
                    handler.register_sse(fd);
                    conn.phase = ConnPhase::Sse {
                        pending: Vec::new(),
                        offset: 0,
                        last: Instant::now(),
                    };
                    return false;
                }
                return true;
            }
            let written = match poll.write(fd, &buf[*offset..]) {
                Ok(w) => w,
                Err(_) => return true,
            };
            *last = Instant::now();
            if written == 0 {
                return false;
            }
            *offset += written;
            if *offset >= buf.len() {
                if *sse_after {
                    handler.register_sse(fd);
                    conn.phase = ConnPhase::Sse {
                        pending: Vec::new(),
                        offset: 0,
                        last: Instant::now(),
                    };
                    return false;
                }
                return true;
            }
            false
        }
        ConnPhase::Sse {
            pending,
            offset,
            last,
        } => {
            if !ev.writable || *offset >= pending.len() {
                return false;
            }
            let written = match poll.write(fd, &pending[*offset..]) {
                Ok(w) => w,
                Err(_) => return true,
            };
            *last = Instant::now();
            if written == 0 {
                return false;
            }
            *offset += written;
            if *offset >= pending.len() {
                pending.clear();
                *offset = 0;
                let _ = poll.mod_interest(fd, false);
            }
            false
        }
    }
}
