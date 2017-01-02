use std::os::unix::io::{RawFd, IntoRawFd, FromRawFd};

use std::fs::{self, File};
use std::net::{self, TcpStream, TcpListener, UdpSocket};
use std::os::unix::net::{UnixStream, UnixListener, UnixDatagram};
use std::os::unix;

use libc::c_int;

pub enum FdImplementor {
    File(File),
    TcpStream(TcpStream),
    TcpListener(TcpListener),
    UdpSocket(UdpSocket),
    UnixStream(UnixStream),
    UnixListener(UnixListener),
    UnixDatagram(UnixDatagram),
}

#[doc(hidden)]
impl FdImplementor {
    pub fn from(fdtype: c_int, rawfd: c_int) -> Option<FdImplementor> {
        unsafe {
            Some(match fdtype {
                0 => FdImplementor::File(fs::File::from_raw_fd(rawfd)),
                1 => FdImplementor::TcpStream(net::TcpStream::from_raw_fd(rawfd)),
                2 => FdImplementor::TcpListener(net::TcpListener::from_raw_fd(rawfd)),
                3 => FdImplementor::UdpSocket(net::UdpSocket::from_raw_fd(rawfd)),
                4 => FdImplementor::UnixStream(unix::net::UnixStream::from_raw_fd(rawfd)),
                5 => FdImplementor::UnixListener(unix::net::UnixListener::from_raw_fd(rawfd)),
                6 => FdImplementor::UnixDatagram(unix::net::UnixDatagram::from_raw_fd(rawfd)),
                _ => return None,
            })
        }
    }

    pub fn to(self) -> (RawFd, c_int) {
        match self {
            FdImplementor::File(obj)         => (obj.into_raw_fd(), 0),
            FdImplementor::TcpStream(obj)    => (obj.into_raw_fd(), 1),
            FdImplementor::TcpListener(obj)  => (obj.into_raw_fd(), 2),
            FdImplementor::UdpSocket(obj)    => (obj.into_raw_fd(), 3),
            FdImplementor::UnixStream(obj)   => (obj.into_raw_fd(), 4),
            FdImplementor::UnixListener(obj) => (obj.into_raw_fd(), 5),
            FdImplementor::UnixDatagram(obj) => (obj.into_raw_fd(), 6),
        }
    }
}
