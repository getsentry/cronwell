#![feature(question_mark, custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate serde;
extern crate serde_json;
extern crate base64;
extern crate clap;
extern crate curl;
extern crate libc;
extern crate url;

mod macros;

mod api;
mod cli;
mod error;
mod monitorid;
mod processtools;
mod utils;


fn main() {
    if let Err(ref err) = cli::execute() {
        err.exit();
    }
}
