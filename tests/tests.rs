#![feature(fs, fs_ext, path_ext, fs_time, fs_walk)]

extern crate rustc_serialize;
extern crate libmeg;
extern crate hamcrest;
extern crate term;
extern crate url;
extern crate tempdir;

#[macro_use]
extern crate log;

mod support;
macro_rules! test {
    ($name:ident $expr:expr) => (
        #[test]
        fn $name() {
/**         we don't have any    
            ::support::paths::setup();
            setup(); **/
            $expr;
        }
    )
}

mod test_shell;
