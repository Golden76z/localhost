use super::handle::SocketFd;
use libc::{self, c_int, sockaddr_in, sockaddr_storage, socklen_t};
use std::io;
use std::mem;
use std::net::{Ipv4Addr, SocketAddrV4};

pub fn set_nonblocking(fd: SocketFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn set_reuseaddr(fd: SocketFd) -> io::Result<()> {
    let on: c_int = 1;
    let rc = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &on as *const _ as *const libc::c_void,
            mem::size_of_val(&on) as socklen_t,
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn create_listener(addr: &SocketAddrV4) -> io::Result<SocketFd> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    set_reuseaddr(fd)?;
    set_nonblocking(fd)?;

    let mut sa: sockaddr_in = unsafe { mem::zeroed() };
    sa.sin_family = libc::AF_INET as _;
    sa.sin_port = addr.port().to_be();
    let octets = addr.ip().octets();
    sa.sin_addr.s_addr = u32::from_ne_bytes(octets).to_be();

    let rc = unsafe {
        libc::bind(
            fd,
            &sa as *const _ as *const libc::sockaddr,
            mem::size_of_val(&sa) as socklen_t,
        )
    };
    if rc < 0 {
        unsafe { libc::close(fd) };
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::listen(fd, 128) } < 0 {
        unsafe { libc::close(fd) };
        return Err(io::Error::last_os_error());
    }
    Ok(fd)
}

pub fn accept(fd: SocketFd) -> io::Result<(SocketFd, SocketAddrV4)> {
    let mut storage: sockaddr_storage = unsafe { mem::zeroed() };
    let mut len = mem::size_of_val(&storage) as socklen_t;
    let client = unsafe {
        libc::accept(
            fd,
            &mut storage as *mut _ as *mut libc::sockaddr,
            &mut len,
        )
    };
    if client < 0 {
        return Err(io::Error::last_os_error());
    }
    set_nonblocking(client)?;

    let sa = unsafe { *(&storage as *const _ as *const sockaddr_in) };
    let ip = Ipv4Addr::from(u32::from_be(sa.sin_addr.s_addr));
    let port = u16::from_be(sa.sin_port);
    Ok((client, SocketAddrV4::new(ip, port)))
}

pub fn close_fd(fd: SocketFd) {
    unsafe { libc::close(fd) };
}

pub fn read_fd(fd: SocketFd, buf: &mut [u8]) -> io::Result<usize> {
    let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(n as usize)
}

pub fn write_fd(fd: SocketFd, buf: &[u8]) -> io::Result<usize> {
    let n = unsafe { libc::write(fd, buf.as_ptr() as *const _, buf.len()) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(n as usize)
}

pub fn is_would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}
