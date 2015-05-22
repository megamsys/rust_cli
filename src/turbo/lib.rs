#![deny(unused)]
#![cfg_attr(test, deny(warnings))]

#[macro_use] extern crate log;

#[cfg(test)] extern crate hamcrest;
extern crate docopt;
extern crate glob;
extern crate rustc_serialize;
extern crate term;
extern crate time;
extern crate libc;

pub mod util;
pub mod core;
pub mod turbo;
