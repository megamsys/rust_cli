use std::error::Error;
use std::fmt;
use std::io;
use std::process::{Output, ExitStatus};
use std::str;

use rustc_serialize::json;

pub type TurboResult<T> = Result<T, Box<TurboError>>;

// =============================================================================
// TurboError trait

pub trait TurboError: Error + Send + 'static {
    fn is_human(&self) -> bool { false }
    fn turbo_cause(&self) -> Option<&TurboError>{ None }
}

impl Error for Box<TurboError> {
    fn description(&self) -> &str { (**self).description() }
    fn cause(&self) -> Option<&Error> { (**self).cause() }
}

impl TurboError for Box<TurboError> {
    fn is_human(&self) -> bool { (**self).is_human() }
    fn turbo_cause(&self) -> Option<&TurboError> { (**self).turbo_cause() }
}

// =============================================================================
// Chaining errors

pub trait ChainError<T> {
    fn chain_error<E, F>(self, callback: F) -> TurboResult<T>
                         where E: TurboError, F: FnOnce() -> E;
}

#[derive(Debug)]
struct ChainedError<E> {
    error: E,
    cause: Box<TurboError>,
}

impl<'a, T, F> ChainError<T> for F where F: FnOnce() -> TurboResult<T> {
    fn chain_error<E, C>(self, callback: C) -> TurboResult<T>
                         where E: TurboError, C: FnOnce() -> E {
        self().chain_error(callback)
    }
}

impl<T, E: TurboError + 'static> ChainError<T> for Result<T, E> {
    #[allow(trivial_casts)]
    fn chain_error<E2: 'static, C>(self, callback: C) -> TurboResult<T>
                         where E2: TurboError, C: FnOnce() -> E2 {
        self.map_err(move |err| {
            Box::new(ChainedError {
                error: callback(),
                cause: Box::new(err),
            }) as Box<TurboError>
        })
    }
}

impl<T> ChainError<T> for Box<TurboError> {
    fn chain_error<E2, C>(self, callback: C) -> TurboResult<T>
                         where E2: TurboError, C: FnOnce() -> E2 {
        Err(Box::new(ChainedError {
            error: callback(),
            cause: self,
        }))
    }
}

impl<T> ChainError<T> for Option<T> {
    fn chain_error<E: 'static, C>(self, callback: C) -> TurboResult<T>
                         where E: TurboError, C: FnOnce() -> E {
        match self {
            Some(t) => Ok(t),
            None => Err(Box::new(callback())),
        }
    }
}

impl<E: Error> Error for ChainedError<E> {
    fn description(&self) -> &str { self.error.description() }
}

impl<E: fmt::Display> fmt::Display for ChainedError<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl<E: TurboError> TurboError for ChainedError<E> {
    fn is_human(&self) -> bool { self.error.is_human() }
    fn turbo_cause(&self) -> Option<&TurboError> { Some(&*self.cause) }
}

// =============================================================================
// Process errors

pub struct ProcessError {
    pub desc: String,
    pub exit: Option<ExitStatus>,
    pub output: Option<Output>,
    cause: Option<io::Error>,
}

impl Error for ProcessError {
    fn description(&self) -> &str { &self.desc }
    #[allow(trivial_casts)]
    fn cause(&self) -> Option<&Error> {
        self.cause.as_ref().map(|s| s as &Error)
    }
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.desc, f)
    }
}
impl fmt::Debug for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// =============================================================================
// Concrete errors

struct ConcreteTurboError {
    description: String,
    detail: Option<String>,
    cause: Option<Box<Error+Send>>,
    is_human: bool,
}

impl fmt::Display for ConcreteTurboError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{}", self.description));
        if let Some(ref s) = self.detail {
            try!(write!(f, " ({})", s));
        }
        Ok(())
    }
}
impl fmt::Debug for ConcreteTurboError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Error for ConcreteTurboError {
    fn description(&self) -> &str { &self.description }
    fn cause(&self) -> Option<&Error> {
        self.cause.as_ref().map(|c| {
            let e: &Error = &**c; e
        })
    }
}

impl TurboError for ConcreteTurboError {
    fn is_human(&self) -> bool {
        self.is_human
    }
}

// =============================================================================
// Human errors

#[derive(Debug)]
pub struct Human<E>(pub E);

impl<E: Error> Error for Human<E> {
    fn description(&self) -> &str { self.0.description() }
    fn cause(&self) -> Option<&Error> { self.0.cause() }
}

impl<E: fmt::Display> fmt::Display for Human<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<E: TurboError> TurboError for Human<E> {
    fn is_human(&self) -> bool { true }
    fn turbo_cause(&self) -> Option<&TurboError> { self.0.turbo_cause() }
}

// =============================================================================
// CLI errors

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub struct CliError {
    pub error: Box<TurboError>,
    pub unknown: bool,
    pub exit_code: i32
}

impl Error for CliError {
    fn description(&self) -> &str { self.error.description() }
    fn cause(&self) -> Option<&Error> { self.error.cause() }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl CliError {
    pub fn new(error: &str, code: i32) -> CliError {
        let error = human(error.to_string());
        CliError::from_boxed(error, code)
    }

    pub fn from_error<E: TurboError>(error: E, code: i32) -> CliError {
        let error = Box::new(error);
        CliError::from_boxed(error, code)
    }

    pub fn from_boxed(error: Box<TurboError>, code: i32) -> CliError {
        let human = error.is_human();
        CliError { error: error, exit_code: code, unknown: !human }
    }
}

impl From<Box<TurboError>> for CliError {
    fn from(err: Box<TurboError>) -> CliError {
        CliError::from_boxed(err, 101)
    }
}

// =============================================================================
// various impls

macro_rules! from_error {
    ($($p:ty,)*) => (
        $(impl From<$p> for Box<TurboError> {
            fn from(t: $p) -> Box<TurboError> { Box::new(t) }
        })*
    )
}

from_error! {
    io::Error,
    ProcessError,
    json::DecoderError,
    CliError,
}

impl<E: TurboError> From<Human<E>> for Box<TurboError> {
    fn from(t: Human<E>) -> Box<TurboError> { Box::new(t) }
}

impl TurboError for io::Error {}
impl TurboError for json::DecoderError {}
impl TurboError for ProcessError {}
impl TurboError for CliError {}

// =============================================================================
// Construction helpers

pub fn process_error(msg: &str,
                     cause: Option<io::Error>,
                     status: Option<&ExitStatus>,
                     output: Option<&Output>) -> ProcessError {
    let exit = match status {
        Some(s) => s.to_string(),
        None => "never executed".to_string(),
    };
    let mut desc = format!("{} ({})", &msg, exit);

    if let Some(out) = output {
        match str::from_utf8(&out.stdout) {
            Ok(s) if s.trim().len() > 0 => {
                desc.push_str("\n--- stdout\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
        match str::from_utf8(&out.stderr) {
            Ok(s) if s.trim().len() > 0 => {
                desc.push_str("\n--- stderr\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
    }

    ProcessError {
        desc: desc,
        exit: status.map(|a| a.clone()),
        output: output.map(|a| a.clone()),
        cause: cause,
    }
}

pub fn internal_error(error: &str, detail: &str) -> Box<TurboError> {
    Box::new(ConcreteTurboError {
        description: error.to_string(),
        detail: Some(detail.to_string()),
        cause: None,
        is_human: false
    })
}

pub fn internal<S: fmt::Display>(error: S) -> Box<TurboError> {
    Box::new(ConcreteTurboError {
        description: error.to_string(),
        detail: None,
        cause: None,
        is_human: false
    })
}

pub fn human<S: fmt::Display>(error: S) -> Box<TurboError> {
    Box::new(ConcreteTurboError {
        description: error.to_string(),
        detail: None,
        cause: None,
        is_human: true
    })
}

pub fn caused_human<S, E>(error: S, cause: E) -> Box<TurboError>
    where S: fmt::Display,
          E: Error + Send + 'static
{
    Box::new(ConcreteTurboError {
        description: error.to_string(),
        detail: None,
        cause: Some(Box::new(cause)),
        is_human: true
    })
}
