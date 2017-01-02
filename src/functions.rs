use std::mem::{size_of, size_of_val, zeroed};
use std::os::unix::io::AsRawFd;

use std::os::unix::net::UnixStream;

use libc::{self, c_void, c_int, cmsghdr, iovec, msghdr, SOL_SOCKET};

#[cfg(not(target_os = "macos"))]
use libc::__errno_location;
#[cfg(target_os = "macos")]
use libc::__error as __errno_location;

use FdImplementor;
use utils::{compute_bufspace, compute_msglen};
#[cfg(any(debug, test))]
use utils::dump_msg;

extern "C" {
    #[doc(hidden)]
    fn get_SCM_RIGHTS() -> c_int;
}

macro_rules! auto_cast {
    ($right:expr) => {{
        #[cfg(not(target_os = "macos"))]
        {
            $right
        }
        #[cfg(target_os = "macos")]
        {
            $right as libc::c_uint
        }
    }};
    ($right:expr, $cast:ty) => {{
        #[cfg(not(target_os = "macos"))]
        {
            $right
        }
        #[cfg(target_os = "macos")]
        {
            $right as $cast
        }
    }}
}

// TODO: impl this as a method to FdImplementor or UnixStream.
pub fn send(channel: &UnixStream, wrapped: FdImplementor) -> Result<(), String> {
    let (rawfd, mut fdtype) = wrapped.to();

    let mut controlbuf = vec![0u8; compute_bufspace(size_of::<c_int>())];
    let mut iov : iovec = unsafe { zeroed() };
    let mut message : msghdr = unsafe { zeroed() };

    iov.iov_base = &mut fdtype as *mut i32 as *mut c_void;
    iov.iov_len = size_of_val(&fdtype);

    message.msg_control = controlbuf.as_mut_ptr() as *mut c_void;
    message.msg_controllen = auto_cast!(controlbuf.len());
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;

    unsafe {
        let controlp : *mut cmsghdr = message.msg_control as *mut cmsghdr;
        (*controlp).cmsg_level = SOL_SOCKET;
        (*controlp).cmsg_type = get_SCM_RIGHTS();
        (*controlp).cmsg_len = auto_cast!(compute_msglen(size_of::<c_int>()));

        let datap : *mut c_int = controlp.offset(1) as *mut c_int; 
        *datap = rawfd;

        #[cfg(any(debug, test))]
        dump_msg(&message);

        match libc::sendmsg(channel.as_raw_fd(), &mut message, 0) {
            x if x == size_of::<c_int>() as isize => Ok(()),
            -1 => {
                let s = libc::strerror(*__errno_location());
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

// TODO: impl this as a method to FdImplementor or UnixStream.
pub fn receive(channel: &UnixStream) -> Result<FdImplementor, String> {
    let mut fdtype : c_int = -1;
    let mut controlbuf = vec![0u8; compute_bufspace(size_of::<c_int>())];
    let mut iov : iovec = unsafe { zeroed() };
    let mut message : msghdr = unsafe { zeroed() };

    iov.iov_base = &mut fdtype as *mut i32 as *mut c_void;
    iov.iov_len = size_of::<c_int>();

    message.msg_control = controlbuf.as_mut_ptr() as *mut c_void;
    message.msg_controllen = auto_cast!(size_of_val(&controlbuf));
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;

    unsafe {
        let read = libc::recvmsg(channel.as_raw_fd(), &mut message, 0);
        match read {
            x if x == size_of::<c_int>() as isize => {
                let controlp : *mut cmsghdr =
                    if message.msg_controllen >= auto_cast!(size_of::<cmsghdr>()) {
                        message.msg_control as *mut cmsghdr
                    } else {
                        ::std::ptr::null_mut()
                    };
                // The cmsghdr struct is made so that multiple ones can be chained.
                // Here we ensure that we only received one, otherwise we fail
                // explicitly to ensure consistency with the provided server
                // API (which only sends one int packed with only one cmsghdr struct).
                if (*controlp).cmsg_level != libc::SOL_SOCKET
                   || (*controlp).cmsg_type != get_SCM_RIGHTS() {
                    return Err("Message was not the expected command: format mismatch".to_owned());
                }
                if message.msg_controllen > auto_cast!(compute_bufspace(size_of::<c_int>())) {
                    return Err("Message read was longer than expected: format mismatch".to_owned());
                }
                if message.msg_controllen < auto_cast!(compute_bufspace(size_of::<c_int>())) {
                    return Err("Message read was shorter than expected: format mismatch".to_owned());
                }
                let rawfd = *((message.msg_control as *mut cmsghdr).offset(1) as *mut c_int);
                FdImplementor::from(fdtype, rawfd).ok_or("Unexpected file descriptor type".to_owned())
            },
            -1 => {
                let s = libc::strerror(*__errno_location());
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
    extern crate tempdir;

    use std::io::{Write, Read};
    use std::os::unix::net::{UnixStream, UnixListener};
    use std::fs::{self, remove_file};
    use std::{thread, time};
    use ::{FdImplementor, receive, send};
    use std::path::Path;
    use std::fmt::Debug;
    use self::tempdir::TempDir;

    #[test]
    fn run() {
        let tmp_dir = TempDir::new("tmp").expect("create temp dir");
        let sockpath = tmp_dir.path().join("rust-fd-passing.test.sock");
        let fpath = tmp_dir.path().join("rust-fd-passing.test.txt");
        let lineone = String::from("Imperio is such an ass!\n");
        let linetwo = String::from("So true...\n");
        let mut text = String::new();
        text.push_str(&lineone);
        text.push_str(&linetwo);
        cleanup();
        unsafe {
            let pid = ::libc::fork();
            match pid {
                -1 => assert!(false, "fork failed"),
                0 => {
                    // If we don't "forget" tmp_dir in here, the folder is removed before the
                    // parent has ended.
                    ::std::mem::forget(tmp_dir);
                    run_child(&sockpath, &linetwo);
                }
                _ => {
                    run_parent(&sockpath, &fpath, &lineone, &text);
                    // Seems everything ran as expected, we can remove the logs.
                    cleanup();
                }
            };
        }
    }

    fn run_child<S: AsRef<Path>>(sockpath: &S, linetwo: &str) {
        let res = run_fd_receiver(&sockpath, linetwo);
        if !res.is_ok() {
            panic!(res.err().unwrap());
        }
    }

    fn run_parent<S: AsRef<Path> + Debug>(sockpath: &S, fpath: &S, lineone: &str, text: &str) {
        thread::sleep(time::Duration::new(1, 0));
        let res = run_fd_sender(sockpath, fpath, lineone);
        if !res.is_ok() {
            panic!(res.err().unwrap());
        }
        thread::sleep(time::Duration::new(1, 0));
        let mut f = fs::File::open(fpath).expect(&format!("cannot open {:?}", fpath));
        let mut readstr = String::new();
        let bytes = f.read_to_string(&mut readstr).unwrap();
        assert!(bytes == text.len(), "Resulting data was not of the expected size.");
        assert!(readstr == text, "Resulting data differs from expectations.");
    }

    #[allow(unused_must_use)]
    fn cleanup() {
        remove_file("/tmp/rust-fd-passing-child-log.txt");
    }

    fn printfile(text: &str) {
        let fpath = String::from("/tmp/rust-fd-passing-child-log.txt");
        let mut f = fs::OpenOptions::new().append(true).create(true).open(&fpath)
                                                                    .expect("printfile failed");
        let written = f.write_all(text.as_bytes());
        assert!(written.is_ok());
    }

    fn run_fd_receiver<S: AsRef<Path>>(sockpath: &S, text: &str) -> Result<bool, String> {
        let listener = UnixListener::bind(sockpath).unwrap();
        printfile("Started server\n");
        // accept one connection and process it, receiving the fd and reading it
        let stream = listener.incoming().next().unwrap();
        match stream {
            Ok(stream) => {
                printfile("Accepted client\n");
                /* connection succeeded */
                match receive(&stream) {
                    Ok(FdImplementor::File(mut res)) => {
                        printfile("Writing into file\n");
                        res.write_all(text.as_bytes())
                           .map_err(|_| "Could not write second data line.")?;
                        Ok(true)
                    },
                    Err(e) => Err(e),
                    _ => Err("Did not get the expected FdImplementor type.".to_owned()),
                }
            }
            Err(e) => Err(format!("IO Error: {}", e))
        }
    }

    fn run_fd_sender<S: AsRef<Path> + Debug>(sockpath: &S, fpath: &S, text: &str) -> Result<bool, String> {
        let mut f = fs::File::create(fpath)
                             .expect(&format!("Could not create data file {:?}", fpath));
        f.write_all(text.as_bytes()).expect("Could not write first data line.");
        let stream = UnixStream::connect(sockpath)
                                .expect(&format!("cannot connect to unix socket {:?}", sockpath));
        send(&stream, FdImplementor::File(f))?;
        Ok(true)
    }
}
