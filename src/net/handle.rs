/// Socket identifier in the poll loop (OS fd on Unix, table index on Windows).
#[cfg(unix)]
pub type SocketFd = std::os::fd::RawFd;

#[cfg(windows)]
pub type SocketFd = usize;
