#![feature(fs, fs_ext, path_ext, fs_time, fs_walk)]

extern crate rustc_serialize;
extern crate libturbo;
extern crate hamcrest;
extern crate term;
extern crate tempdir;

#[macro_use]
extern crate log;

mod support;
macro_rules! test {
    ($name:ident $expr:expr) => (
        #[test]
        fn $name() {
            $expr;
        }
    )
}

mod test_shell;
