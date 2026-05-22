mod handle;
mod poll_common;

pub use handle::SocketFd;
pub use poll_common::{IoEvent, IoKind};

#[cfg(unix)]
mod epoll;
#[cfg(unix)]
mod socket_unix;

#[cfg(windows)]
mod poll_std;

#[cfg(unix)]
pub use epoll::{timed_out, IoLoop};
#[cfg(windows)]
pub use poll_std::{timed_out, IoLoop};

pub type EpollLoop = IoLoop;

pub fn is_would_block(err: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        err.kind() == std::io::ErrorKind::WouldBlock
    }
    #[cfg(windows)]
    {
        err.kind() == std::io::ErrorKind::WouldBlock || err.kind() == std::io::ErrorKind::TimedOut
    }
}
