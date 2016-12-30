use std::mem::size_of;

use libc::cmsghdr;

#[cfg(any(not(target_os = "macos"), debug, test))]
use libc;

// XXX TODO FIXME TODO XXX
// Make the three following functions into a static object initialized only
// once with the required values.
// XXX TODO FIXME TODO XXX
#[cfg(not(target_os = "macos"))]
fn compute_aligned(basesz: usize) -> usize {
    let mod_ = basesz % size_of::<libc::size_t>();
    match mod_ {
        0 => basesz,
        _ => basesz + (size_of::<libc::size_t>() - mod_),
    }
}

#[cfg(target_os = "macos")]
fn compute_aligned(basesz: usize) -> usize {
    let mod_ = basesz % size_of::<u32>();
    match mod_ {
        0 => basesz,
        _ => basesz + (size_of::<u32>() - mod_),
    }
}

pub fn compute_bufspace(len: usize) -> usize {
    compute_aligned(len) + compute_aligned(size_of::<cmsghdr>())
}

pub fn compute_msglen(len: usize) -> usize {
    compute_aligned(size_of::<cmsghdr>()) + len
}

#[cfg(any(debug, test))]
pub fn dump_msg(msg: &libc::msghdr) {
    let data = msg as *const libc::msghdr as *const libc::c_void as *const libc::c_char;
    let sz = ::std::mem::size_of_val(msg);
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

        println!("msg.control:    {:?}", ctrlp);
        println!("msg.controllen: {}",   msg.msg_controllen);
        println!("msg.iov:        {:?}", msg.msg_iov as *const libc::c_void);
        println!("msg.iovlen:     {}",   msg.msg_iovlen);

        let ctrl = *ctrlp;
        println!("ctrl.level:     {}", ctrl.cmsg_level);
        println!("ctrl.type:      {}", ctrl.cmsg_type);
        println!("ctrl.len:       {}", ctrl.cmsg_len);
    }
}
