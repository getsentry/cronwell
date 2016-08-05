use std::io;
use std::io::Write;
use std::fmt;
use std::process;
use std::error;

use api;

use clap;

#[derive(Debug)]
pub struct Error {
    repr: ErrorRepr,
}

#[derive(Debug)]
enum ErrorRepr {
    ClapError(clap::Error),
    BasicError(String),
}

macro_rules! basic_error {
    ($ty:ty, $msg:expr) => {
        impl From<$ty> for Error {
            fn from(err: $ty) -> Error {
                Error {
                    repr: ErrorRepr::BasicError(format!("{}: {}", $msg, err))
                }
            }
        }
    }
}

basic_error!(io::Error, "io error");
basic_error!(api::Error, "could not perform API request");

impl From<clap::Error> for Error {
    fn from(err: clap::Error) -> Error {
        Error {
            repr: ErrorRepr::ClapError(err)
        }
    }
}

impl From<String> for Error {
    fn from(err: String) -> Error {
        Error {
            repr: ErrorRepr::BasicError(err)
        }
    }
}

impl<'a> From<&'a str> for Error {
    fn from(err: &'a str) -> Error {
        Error {
            repr: ErrorRepr::BasicError(err.to_owned())
        }
    }
}

impl Error {

    /// Exists the process and prints out the error if needed.
    pub fn exit(&self) -> ! {
        match self.repr {
            ErrorRepr::ClapError(ref err) => err.exit(),
            _ => {
                writeln!(&mut io::stderr(), "error: {}", self).ok();
                process::exit(1)
            },
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.repr {
            ErrorRepr::BasicError(ref msg) => write!(f, "{}", msg),
            ErrorRepr::ClapError(ref err) => write!(f, "{}", err),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self.repr {
            ErrorRepr::BasicError(ref msg) => &msg,
            ErrorRepr::ClapError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self.repr {
            ErrorRepr::ClapError(ref err) => Some(&*err),
            _ => None,
        }
    }
}
