use super::handle::SocketFd;
use super::poll_common::{IoEvent, IoKind};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddrV4, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

pub use super::poll_common::timed_out;

enum Entry {
    Listener(TcpListener),
    Client(TcpStream),
}

/// Windows/dev backend: one poll scan per iteration (use Linux build for audit/epoll).
pub struct IoLoop {
    entries: Vec<Option<Entry>>,
    kinds: HashMap<SocketFd, IoKind>,
    want_write: HashMap<SocketFd, bool>,
    free: Vec<SocketFd>,
}

impl IoLoop {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            entries: Vec::new(),
            kinds: HashMap::new(),
            want_write: HashMap::new(),
            free: Vec::new(),
        })
    }

    fn alloc(&mut self, entry: Entry, kind: IoKind, interest_out: bool) -> SocketFd {
        let id = self.free.pop().unwrap_or_else(|| {
            self.entries.push(None);
            self.entries.len() - 1
        });
        self.entries[id] = Some(entry);
        self.kinds.insert(id, kind);
        self.want_write.insert(id, interest_out);
        id
    }

    pub fn bind(&mut self, addr: &SocketAddrV4) -> Result<SocketFd, String> {
        let listener = TcpListener::bind(*addr).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(self.alloc(Entry::Listener(listener), IoKind::Listener, false))
    }

    pub fn mod_interest(&mut self, fd: SocketFd, interest_out: bool) -> Result<(), String> {
        if !self.kinds.contains_key(&fd) {
            return Err("fd not registered".into());
        }
        self.want_write.insert(fd, interest_out);
        Ok(())
    }

    pub fn remove(&mut self, fd: SocketFd) {
        self.entries[fd] = None;
        self.kinds.remove(&fd);
        self.want_write.remove(&fd);
        self.free.push(fd);
    }

    pub fn accept(&mut self, listener: SocketFd) -> Result<Option<(SocketFd, SocketAddrV4)>, io::Error> {
        let Entry::Listener(l) = self.entries[listener].as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "not a listener")
        })? else {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "not a listener"));
        };
        match l.accept() {
            Ok((stream, addr)) => {
                stream.set_nonblocking(true)?;
                let v4 = match addr {
                    std::net::SocketAddr::V4(a) => a.clone(),
                    std::net::SocketAddr::V6(a) => SocketAddrV4::new(
                        a.ip().to_ipv4_mapped().unwrap_or(std::net::Ipv4Addr::LOCALHOST),
                        a.port(),
                    ),
                };
                let id = self.alloc(Entry::Client(stream), IoKind::Client, false);
                Ok(Some((id, v4)))
            }
            Err(e) if would_block(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn read(&mut self, fd: SocketFd, buf: &mut [u8]) -> Result<Option<usize>, io::Error> {
        let Entry::Client(s) = self.entries[fd].as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "not a client")
        })? else {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "not a client"));
        };
        match s.read(buf) {
            Ok(0) => Ok(None),
            Ok(n) => Ok(Some(n)),
            Err(e) if would_block(&e) => Ok(Some(0)),
            Err(e) => Err(e),
        }
    }

    pub fn write(&mut self, fd: SocketFd, buf: &[u8]) -> Result<usize, io::Error> {
        let Entry::Client(s) = self.entries[fd].as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "not a client")
        })? else {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "not a client"));
        };
        match s.write(buf) {
            Ok(0) => Ok(0),
            Ok(n) => Ok(n),
            Err(e) if would_block(&e) => Ok(0),
            Err(e) => Err(e),
        }
    }

    pub fn wait(&self, timeout_ms: i32) -> Result<Vec<IoEvent>, String> {
        thread::sleep(Duration::from_millis(timeout_ms.max(0) as u64));
        let mut out = Vec::new();
        for (&fd, &kind) in &self.kinds {
            if self.entries[fd].is_none() {
                continue;
            }
            let readable = true;
            let writable = self.want_write.get(&fd).copied().unwrap_or(false);
            out.push(IoEvent {
                fd,
                kind,
                readable,
                writable,
                hangup: false,
            });
        }
        Ok(out)
    }
}

fn would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock || err.kind() == io::ErrorKind::TimedOut
}
