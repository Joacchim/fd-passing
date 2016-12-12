extern crate libc;

use std::mem::{size_of, size_of_val};
use std::os::unix::io::{RawFd, IntoRawFd, AsRawFd, FromRawFd};

use std::fs::{self, File};
use std::net::{self, TcpStream, TcpListener, UdpSocket};
use std::os::unix::net::{UnixStream, UnixListener, UnixDatagram};

use std::os::unix;

use libc::{c_void, c_int};

pub enum FdImplementor {
    File(File),
    TcpStream(TcpStream),
    TcpListener(TcpListener),
    UdpSocket(UdpSocket),
    UnixStream(UnixStream),
    UnixListener(UnixListener),
    UnixDatagram(UnixDatagram),
}

impl FdImplementor {
    fn from(fdtype : c_int, rawfd : c_int) -> Option<FdImplementor> {
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

    fn to(self) -> (RawFd, c_int) {
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

// XXX TODO FIXME TODO XXXX
// Create a c binding to retrieve the given value "dynamically" from the C
// code, as it looked complex to map
// XXX TODO FIXME TODO XXXX
const SCM_RIGHTS : c_int = 0x01;

// XXX TODO FIXME TODO XXX
// Make the three following functions into a static object initialized only
// once with the required values.
// XXX TODO FIXME TODO XXX
fn compute_aligned(basesz: usize) -> usize {
    let mod_ = basesz % size_of::<libc::size_t>();
    match mod_ {
        0 => basesz,
        _ => basesz + (size_of::<libc::size_t>() - mod_),
    }
}

fn compute_bufspace(len: usize) -> usize {
    compute_aligned(len) + compute_aligned(size_of::<libc::cmsghdr>())
}

fn compute_msglen(len: usize) -> usize {
    compute_aligned(size_of::<libc::cmsghdr>()) + len
}

#[cfg(debug)]
fn dump_msg(msg: &libc::msghdr) {
    let data = msg as *const libc::msghdr as *const c_void as *const libc::c_char;
    let sz = size_of_val(msg);
    println!("Dumping msg: buffer of {} bytes in hexa:", sz);
    let mut pos = 0;
    while pos < sz {
        unsafe {
            println!("{:02x}{:02x}{:02x}{:02x} {:02x}{:02x}{:02x}{:02x}",
                     *data.offset((pos as isize)),
                     *data.offset((pos as isize) + 1),
                     *data.offset((pos as isize) + 2),
                     *data.offset((pos as isize) + 3),
                     *data.offset((pos as isize) + 4),
                     *data.offset((pos as isize) + 5),
                     *data.offset((pos as isize) + 6),
                     *data.offset((pos as isize) + 7));
        }
        pos += 8;
    }

    unsafe {
        let ctrlp = msg.msg_control as *const libc::cmsghdr;

        println!("msg.control:    {:?}",     ctrlp);
        println!("msg.controllen: {}",  msg.msg_controllen);
        println!("msg.iov:        {:?}",         msg.msg_iov as *const c_void);
        println!("msg.iovlen:     {}",      msg.msg_iovlen);

        let ctrl = *ctrlp;
        println!("ctrl.level:     {}",  ctrl.cmsg_level);
        println!("ctrl.type:      {}",   ctrl.cmsg_type);
        println!("ctrl.len:       {}",    ctrl.cmsg_len);
    }
}

pub fn send(channel: &UnixStream, wrapped: FdImplementor) -> Result<(), String> {
    let (rawfd, mut fdtype) = wrapped.to();

    let mut controlbuf = vec![0u8; compute_bufspace(size_of::<c_int>())];
    let mut iov : libc::iovec = unsafe { std::mem::zeroed() };
    let mut message : libc::msghdr = unsafe { std::mem::zeroed() };

    iov.iov_base = &mut fdtype as *mut i32 as *mut c_void;
    iov.iov_len = size_of_val(&fdtype);
    
    message.msg_control = controlbuf.as_mut_ptr() as *mut c_void;
    message.msg_controllen = size_of_val(&controlbuf);
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;

    unsafe {
        let controlp : *mut libc::cmsghdr = message.msg_control as *mut libc::cmsghdr;
        (*controlp).cmsg_level = libc::SOL_SOCKET;
        (*controlp).cmsg_type = SCM_RIGHTS;
        (*controlp).cmsg_len = compute_msglen(size_of::<c_int>());

        let datap : *mut c_int = controlp.offset(1) as *mut c_int; 
        *datap = rawfd;

        #[cfg(debug)]
        dump_msg(&message);

        let written = libc::sendmsg(channel.as_raw_fd(), &mut message, 0);
        match written {
             x if x == size_of::<c_int>() as isize => Ok(()),
            -1 => {
                let s = libc::strerror(*libc::__errno_location());
                let slen = libc::strlen(s);
                let serr = String::from_raw_parts(s as *mut u8, slen, slen);
                let rerr = serr.clone();
                ::std::mem::forget(serr);
                Err(rerr)
            },
            _  => Err("Incomplete message sent".to_owned()),
        }
    }
}

pub fn receive(channel: &UnixStream) -> Result<FdImplementor, String> {
    let mut fdtype : c_int = -1;
    let mut controlbuf = vec![0u8; compute_bufspace(size_of::<c_int>())];
    let mut iov : libc::iovec = unsafe { std::mem::zeroed() };
    let mut message : libc::msghdr = unsafe { std::mem::zeroed() };

    iov.iov_base = &mut fdtype as *mut i32 as *mut c_void;
    iov.iov_len = size_of::<c_int>();
    
    message.msg_control = controlbuf.as_mut_ptr() as *mut c_void;
    message.msg_controllen = size_of_val(&controlbuf);
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    
    unsafe {
        let read = libc::recvmsg(channel.as_raw_fd(), &mut message, 0);
        match read {
            x if x == size_of::<c_int>() as isize => {
                let controlp : *mut libc::cmsghdr =
                    if message.msg_controllen >= size_of::<libc::cmsghdr>() {
                        message.msg_control as *mut libc::cmsghdr
                    } else {
                        ::std::ptr::null_mut()
                    };
                // The cmsghdr struct is made so that multiple ones can be chained.
                // Here we ensure that we only received one, otherwise we fail
                // explicitly to ensure consistency with the provided server
                // API (which only sends one int packed with only one cmsghdr struct).
                if (*controlp).cmsg_level != libc::SOL_SOCKET
                   || (*controlp).cmsg_type != SCM_RIGHTS {
                    return Err("Message was not the expected command: format mismatch".to_owned());
                }
                if message.msg_controllen > compute_bufspace(size_of::<c_int>()) {
                    return Err("Message read was longer than expected: format mismatch".to_owned());
                }
                if message.msg_controllen < compute_bufspace(size_of::<c_int>()) {
                    return Err("Message read was shorter than expected: format mismatch".to_owned());
                }
                let rawfd = *((message.msg_control as *mut libc::cmsghdr).offset(1) as *mut c_int);
                FdImplementor::from(fdtype, rawfd).ok_or("Unexpected file descriptor type".to_owned())
            },
            -1 => {
                let s = libc::strerror(*libc::__errno_location());
                let slen = libc::strlen(s);
                let serr = String::from_raw_parts(s as *mut u8, slen, slen);
                let rerr = serr.clone();
                ::std::mem::forget(serr);
                Err(rerr)
            },
            _ => Err("Message data was not of the expected size".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use ::*;
    use std::io::{Write, Read};


    #[test]
    fn run() {
        let sockpath = "/tmp/rust-fd-passing.test.sock";
        let fpath = "/tmp/rust-fd-passing.test.txt";
        let lineone = String::from("Imperio is such an ass!\n");
        let linetwo = String::from("So true...\n");
        let mut text = String::new();
        text.push_str(&lineone);
        text.push_str(&linetwo);
        cleanup(sockpath, fpath);
        unsafe {
            let pid = ::libc::fork();
            match pid {
                -1 => assert!(false, ""),
                0 => {
                    let res = run_fd_receiver(sockpath, &linetwo);
                    assert!(res.is_ok());
                },
                _ => {
                    std::thread::sleep(std::time::Duration::new(1, 0));
                    let res = run_fd_sender(sockpath, fpath, &lineone);
                    assert!(res.is_ok());
                    std::thread::sleep(std::time::Duration::new(1, 0));
                    let mut f = fs::File::open(fpath).unwrap();
                    let mut readstr = String::new();
                    let bytes = f.read_to_string(&mut readstr).unwrap();
                    assert!(bytes == text.len(), "Resulting data was not of the expected size.");
                    assert!(readstr == text, "Resulting data differs from expectations.");
                },
            };
        }
    }

    #[allow(unused_must_use)]
    fn cleanup(sockpath: &str, fpath: &str) {
        std::fs::remove_file(sockpath);
        std::fs::remove_file(fpath);
        std::fs::remove_file("/tmp/rust-fd-passing-child-log.txt");
    }

    fn printfile(text: &str) {
        let fpath = String::from("/tmp/rust-fd-passing-child-log.txt");
        let f = fs::OpenOptions::new().append(true).create(true).open(&fpath).map_err(|_| "Failed");
        assert!(f.is_ok());
        let written = f.unwrap().write_all(text.as_bytes());
        assert!(written.is_ok());
    }

    fn run_fd_receiver(sockpath: &str, text: &str) -> Result<bool, String> {
        let listener = UnixListener::bind(sockpath).unwrap();
        printfile("Started server");
        // accept one connection and process it, receiving the fd and reading it
        let stream = listener.incoming().next().unwrap();
        match stream {
            Ok(stream) => {
                printfile("Accepted client");
                /* connection succeeded */
                match receive(&stream) {
                    Ok(FdImplementor::File(mut res)) => {
                        printfile("Writing into file");
                        res.write_all(text.as_bytes()).map_err(|_| "Could not write second data line.")?;
                        Ok(true)
                    },
                    Err(e) => Err(e),
                    _ => Err("Did not get the expected FdImplementor type.".to_owned()),
                }
            }
            Err(e) => Err(format!("IO Error: {}", e))
        }
    }

    fn run_fd_sender(sockpath: &str, fpath: &str, text: &str) -> Result<bool, String> {
        let mut f = fs::File::create(fpath).map_err(|_| "Could not create data file")?;
        f.write_all(text.as_bytes()).map_err(|_| "Could not write first data line.".to_owned())?;
        let stream = UnixStream::connect(sockpath).unwrap();
        send(&stream, FdImplementor::File(f))?;
        Ok(true)
    }
}
