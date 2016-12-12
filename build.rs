extern crate gcc;

fn main() {
    gcc::compile_library("libfd_passing_consts.a", &["ffi/consts.c"])
}
