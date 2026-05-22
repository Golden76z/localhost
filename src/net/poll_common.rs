use super::handle::SocketFd;
use std::time::{Duration, Instant};

pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoKind {
    Listener,
    Client,
}

#[derive(Debug)]
pub struct IoEvent {
    pub fd: SocketFd,
    pub kind: IoKind,
    pub readable: bool,
    pub writable: bool,
    pub hangup: bool,
}

pub fn timed_out(last: Instant) -> bool {
    Instant::now().duration_since(last) > REQUEST_TIMEOUT
}
