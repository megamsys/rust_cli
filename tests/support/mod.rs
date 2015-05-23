extern crate libturbo;

use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::str;

use hamcrest as ham;
use turbo::util::{process,ProcessBuilder};
use turbo::util::ProcessError;



/// Returns an absolute path in the filesystem that `path` points to. The
/// returned path does not contain any symlinks in its hierarchy.
/*
 *
 * ===== Matchers =====
 *
 */

#[derive(Clone)]
pub struct Execs {
    expect_stdout: Option<String>,
    expect_stdin: Option<String>,
    expect_stderr: Option<String>,
    expect_exit_code: Option<i32>
}

impl Execs {

    pub fn with_stdout<S: ToString>(mut self, expected: S) -> Execs {
        self.expect_stdout = Some(expected.to_string());
        self
    }

    pub fn with_stderr<S: ToString>(mut self, expected: S) -> Execs {
        self.expect_stderr = Some(expected.to_string());
        self
    }

    pub fn with_status(mut self, expected: i32) -> Execs {
        self.expect_exit_code = Some(expected);
        self
    }

    fn match_output(&self, actual: &Output) -> ham::MatchResult {
        self.match_status(actual)
            .and(self.match_stdout(actual))
            .and(self.match_stderr(actual))
    }

    fn match_status(&self, actual: &Output) -> ham::MatchResult {
        match self.expect_exit_code {
            None => ham::success(),
            Some(code) => {
                ham::expect(
                    actual.status.code() == Some(code),
                    format!("exited with {}\n--- stdout\n{}\n--- stderr\n{}",
                            actual.status,
                            String::from_utf8_lossy(&actual.stdout),
                            String::from_utf8_lossy(&actual.stderr)))
            }
        }
    }

    fn match_stdout(&self, actual: &Output) -> ham::MatchResult {
        self.match_std(self.expect_stdout.as_ref(), &actual.stdout,
                       "stdout", &actual.stderr)
    }

    fn match_stderr(&self, actual: &Output) -> ham::MatchResult {
        self.match_std(self.expect_stderr.as_ref(), &actual.stderr,
                       "stderr", &actual.stdout)
    }

    fn match_std(&self, expected: Option<&String>, actual: &[u8],
                 description: &str, extra: &[u8]) -> ham::MatchResult {
        match expected.map(|s| &s[..]) {
            None => ham::success(),
            Some(out) => {
                let actual = match str::from_utf8(actual) {
                    Err(..) => return Err(format!("{} was not utf8 encoded",
                                               description)),
                    Ok(actual) => actual,
                };
                // Let's not deal with \r\n vs \n on windows...
                let actual = actual.replace("\r", "");
                let actual = actual.replace("\t", "<tab>");

                let a = actual.lines();
                let e = out.lines();

                let diffs = zip_all(a, e).enumerate();
                let diffs = diffs.filter_map(|(i, (a,e))| {
                    match (a, e) {
                        (Some(a), Some(e)) => {
                            if lines_match(&e, &a) {
                                None
                            } else {
                                Some(format!("{:3} - |{}|\n    + |{}|\n", i, e, a))
                            }
                        },
                        (Some(a), None) => {
                            Some(format!("{:3} -\n    + |{}|\n", i, a))
                        },
                        (None, Some(e)) => {
                            Some(format!("{:3} - |{}|\n    +\n", i, e))
                        },
                        (None, None) => panic!("Cannot get here")
                    }
                });

                let diffs = diffs.collect::<Vec<String>>().connect("\n");

                ham::expect(diffs.len() == 0,
                            format!("differences:\n\
                                    {}\n\n\
                                    other output:\n\
                                    `{}`", diffs,
                                    String::from_utf8_lossy(extra)))
            }
        }
    }
}

fn lines_match(expected: &str, mut actual: &str) -> bool {
    for part in expected.split("[..]") {
        match actual.find(part) {
            Some(i) => actual = &actual[i + part.len()..],
            None => {
                return false
            }
        }
    }
    actual.len() == 0 || expected.ends_with("[..]")
}

struct ZipAll<I1: Iterator, I2: Iterator> {
    first: I1,
    second: I2,
}

impl<T, I1: Iterator<Item=T>, I2: Iterator<Item=T>> Iterator for ZipAll<I1, I2> {
    type Item = (Option<T>, Option<T>);
    fn next(&mut self) -> Option<(Option<T>, Option<T>)> {
        let first = self.first.next();
        let second = self.second.next();

        match (first, second) {
            (None, None) => None,
            (a, b) => Some((a, b))
        }
    }
}

fn zip_all<T, I1: Iterator<Item=T>, I2: Iterator<Item=T>>(a: I1, b: I2) -> ZipAll<I1, I2> {
    ZipAll {
        first: a,
        second: b,
    }
}

impl fmt::Display for Execs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "execs")
    }
}

impl ham::Matcher<ProcessBuilder> for Execs {
    fn matches(&self, mut process: ProcessBuilder) -> ham::MatchResult {
        self.matches(&mut process)
    }
}

impl<'a> ham::Matcher<&'a mut ProcessBuilder> for Execs {
    fn matches(&self, process: &'a mut ProcessBuilder) -> ham::MatchResult {
        let res = process.exec_with_output();

        match res {
            Ok(out) => self.match_output(&out),
            Err(ProcessError { output: Some(ref out), .. }) => {
                self.match_output(out)
            }
            Err(e) => {
                let mut s = format!("could not exec process {}: {}", process, e);
                match e.cause() {
                    Some(cause) => s.push_str(&format!("\ncaused by: {}",
                                                       cause.description())),
                    None => {}
                }
                Err(s)
            }
        }
    }
}

pub fn execs() -> Execs {
    Execs {
        expect_stdout: None,
        expect_stderr: None,
        expect_stdin: None,
        expect_exit_code: None
    }
}

#[derive(Clone)]
pub struct ShellWrites {
    expected: String
}

impl fmt::Display for ShellWrites {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "`{}` written to the shell", self.expected)
    }
}

impl<'a> ham::Matcher<&'a [u8]> for ShellWrites {
    fn matches(&self, actual: &[u8])
        -> ham::MatchResult
    {
        let actual = String::from_utf8_lossy(actual);
        let actual = actual.to_string();
        ham::expect(actual == self.expected, actual)
    }
}

pub fn shell_writes<T: fmt::Display>(string: T) -> ShellWrites {
    ShellWrites { expected: string.to_string() }
}

pub trait Tap {
    fn tap<F: FnOnce(&mut Self)>(mut self, callback: F) -> Self;
}

impl<T> Tap for T {
    fn tap<F: FnOnce(&mut Self)>(mut self, callback: F) -> T {
        callback(&mut self);
        self
    }
}

pub fn basic_bin_manifest(name: &str) -> String {
    format!(r#"
        [package]

        name = "{}"
        version = "0.1.0"
        authors = ["abcd@example.com"]

        [[bin]]

        name = "{}"
    "#, name, name)
}

pub fn basic_lib_manifest(name: &str) -> String {
    format!(r#"
        [package]

        name = "{}"
        version = "0.1.0"
        authors = ["abcd@example.com"]

        [lib]

        name = "{}"
    "#, name, name)
}


pub static RUNNING:     &'static str = "     Running";
pub static COMPILING:   &'static str = "   Compiling";
pub static FRESH:       &'static str = "       Fresh";
pub static UPDATING:    &'static str = "    Updating";
pub static DOCTEST:     &'static str = "   Doc-tests";
pub static PACKAGING:   &'static str = "   Packaging";
pub static DOWNLOADING: &'static str = " Downloading";
pub static UPLOADING:   &'static str = "   Uploading";
pub static VERIFYING:   &'static str = "   Verifying";
pub static ARCHIVING:   &'static str = "   Archiving";
