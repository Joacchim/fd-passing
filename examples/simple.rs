extern crate fd_passing;
extern crate libc;
extern crate tempdir;

use std::io::{Write, Read, Seek, SeekFrom, stdin};
use std::os::unix::net::{UnixStream, UnixListener};
use std::fs::OpenOptions;
use std::path::Path;
use std::fmt::Debug;
use std::{thread, time};
use self::tempdir::TempDir;

use fd_passing::FdImplementor;

fn run_child<S: AsRef<Path>>(sockpath: &S) {
    let listener = UnixListener::bind(sockpath).unwrap();

    match listener.incoming().next().unwrap() {
        Ok(stream) => {
            match fd_passing::receive(&stream) {
                Ok(FdImplementor::File(mut res)) => {
                    let mut text = String::new();

                    // We go back to the beginning of the file.
                    res.seek(SeekFrom::Start(0)).expect("couldn't go back to beginning of file");
                    // Then we can read it.
                    res.read_to_string(&mut text).expect("read_to_string call failed");
                    print!("Received a file containing the following text:\n{}", &text);
                },
                Err(e) => panic!(e),
                _ => panic!("Did not get the expected FdImplementor type"),
            }
        }
        e => panic!(e),
    }
}

fn run_parent<S: AsRef<Path> + Debug>(sockpath: &S, fpath: &S, text: &str) {
    let mut f = OpenOptions::new().write(true)
                                  .create(true)
                                  .read(true)
                                  .open(fpath)
                                  .expect(&format!("Could not create data file {:?}", fpath));
    f.write_all(text.as_bytes()).expect("Could not write data");
    println!("Waiting for unix socket creation...");
    thread::sleep(time::Duration::from_millis(500));
    let stream = UnixStream::connect(sockpath)
                            .expect(&format!("cannot connect to unix socket {:?}", sockpath));
    fd_passing::send(&stream, FdImplementor::File(f)).expect("fd_passing::send failed");
}

fn get_input() -> String {
    let stdout = std::io::stdout();
    let mut io = stdout.lock();
    let mut buf = String::new();

    writeln!(io, "Please enter a sentence and press enter:").unwrap();
    write!(io, "> ").unwrap();
    io.flush().unwrap();

    stdin().read_line(&mut buf).expect("read_line failed");
    buf
}

fn main() {
    let tmp_dir = TempDir::new("tmp").expect("create temp dir");
    let sockpath = tmp_dir.path().join("example.sock");
    let fpath = tmp_dir.path().join("example.txt");
    let text_to_send = get_input();
    unsafe {
        let pid = libc::fork();
        match pid {
            -1 => assert!(false, "fork failed"),
            0 => {
                run_child(&sockpath);
            }
            mut x => {
                run_parent(&sockpath, &fpath, &text_to_send);
                println!("waiting for child to finish...");
                libc::wait(&mut x);
                println!("done!");
            }
        };
    }
}
