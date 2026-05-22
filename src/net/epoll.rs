use super::handle::SocketFd;
use super::poll_common::{IoEvent, IoKind};
use super::socket_unix::{self, accept as sys_accept, create_listener, is_would_block};
use libc::{self, epoll_event, EPOLLERR, EPOLLHUP, EPOLLOUT, EPOLLRDHUP, EPOLLET, EPOLLIN};
use std::collections::HashMap;
use std::io;
use std::net::SocketAddrV4;
pub use super::poll_common::timed_out;

const MAX_EVENTS: usize = 64;

pub struct IoLoop {
    epfd: SocketFd,
    registry: HashMap<SocketFd, IoKind>,
    want_write: HashMap<SocketFd, bool>,
}

impl IoLoop {
    pub fn new() -> Result<Self, String> {
        let epfd = unsafe { libc::epoll_create1(0) };
        if epfd < 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        Ok(Self {
            epfd,
            registry: HashMap::new(),
            want_write: HashMap::new(),
        })
    }

    pub fn bind(&mut self, addr: &SocketAddrV4) -> Result<SocketFd, String> {
        let fd = create_listener(addr).map_err(|e| e.to_string())?;
        self.register(fd, IoKind::Listener, false)?;
        Ok(fd)
    }

    pub fn register(&mut self, fd: SocketFd, kind: IoKind, interest_out: bool) -> Result<(), String> {
        self.want_write.insert(fd, interest_out);
        self.set_interest(fd, kind, interest_out)
    }

    pub fn mod_interest(&mut self, fd: SocketFd, interest_out: bool) -> Result<(), String> {
        let kind = *self.registry.get(&fd).ok_or("fd not registered")?;
        self.want_write.insert(fd, interest_out);
        self.set_interest(fd, kind, interest_out)
    }

    fn set_interest(&mut self, fd: SocketFd, kind: IoKind, interest_out: bool) -> Result<(), String> {
        let mut ev = epoll_event {
            events: (EPOLLIN | EPOLLRDHUP | EPOLLERR | EPOLLHUP | EPOLLET) as u32
                | if interest_out { EPOLLOUT as u32 } else { 0 },
            u64: fd as u64,
        };
        let op = if self.registry.contains_key(&fd) {
            libc::EPOLL_CTL_MOD
        } else {
            libc::EPOLL_CTL_ADD
        };
        if unsafe { libc::epoll_ctl(self.epfd, op, fd, &mut ev) } < 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        self.registry.insert(fd, kind);
        Ok(())
    }

    pub fn remove(&mut self, fd: SocketFd) {
        let _ = unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut()) };
        self.registry.remove(&fd);
        self.want_write.remove(&fd);
        socket_unix::close_fd(fd);
    }

    pub fn accept(&mut self, listener: SocketFd) -> Result<Option<(SocketFd, SocketAddrV4)>, io::Error> {
        match sys_accept(listener) {
            Ok((client, addr)) => {
                self.register(client, IoKind::Client, false)?;
                Ok(Some((client, addr)))
            }
            Err(e) if is_would_block(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn read(&mut self, fd: SocketFd, buf: &mut [u8]) -> Result<Option<usize>, io::Error> {
        match socket_unix::read_fd(fd, buf) {
            Ok(0) => Ok(None),
            Ok(n) => Ok(Some(n)),
            Err(e) if is_would_block(&e) => Ok(Some(0)),
            Err(e) => Err(e),
        }
    }

    pub fn write(&mut self, fd: SocketFd, buf: &[u8]) -> Result<usize, io::Error> {
        match socket_unix::write_fd(fd, buf) {
            Ok(0) => Ok(0),
            Ok(n) => Ok(n),
            Err(e) if is_would_block(&e) => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// One `epoll_wait` per call — audit requirement on Linux.
    pub fn wait(&self, timeout_ms: i32) -> Result<Vec<IoEvent>, String> {
        let mut events = vec![epoll_event { events: 0, u64: 0 }; MAX_EVENTS];
        let n = unsafe {
            libc::epoll_wait(
                self.epfd,
                events.as_mut_ptr(),
                MAX_EVENTS as i32,
                timeout_ms,
            )
        };
        if n < 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        let mut out = Vec::new();
        for ev in events.iter().take(n as usize) {
            let fd = ev.u64 as SocketFd;
            let kind = match self.registry.get(&fd) {
                Some(k) => *k,
                None => continue,
            };
            let e = ev.events;
            out.push(IoEvent {
                fd,
                kind,
                readable: (e & EPOLLIN as u32) != 0,
                writable: (e & EPOLLOUT as u32) != 0,
                hangup: (e & (EPOLLRDHUP | EPOLLHUP | EPOLLERR) as u32) != 0,
            });
        }
        Ok(out)
    }
}

impl Drop for IoLoop {
    fn drop(&mut self) {
        socket_unix::close_fd(self.epfd);
    }
}
