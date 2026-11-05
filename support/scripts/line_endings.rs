#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
---
//! Shows or sets text files' line endings, changing nothing else, to put back a file's endings after a tool changed
//! them, such as `rustfmt`, whose `newline_style` is `Native`, or an editor that writes whole files.
//!
//! Note a file's endings, with `git ls-files --eol <file>` or `show`, before changing it, and set them back after.
//!
//! ```text
//! cargo +nightly -Zscript support/scripts/line_endings.rs show planning/stores/src/bom.rs
//! cargo +nightly -Zscript support/scripts/line_endings.rs set lf planning/stores/src/bom.rs
//! ```

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(about = "Shows or sets text files' line endings, LF or CRLF, changing nothing else")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Says how many lines of each file end with LF, and how many with CRLF.
    Show { files: Vec<PathBuf> },
    /// Makes every line of each file end with the ending, leaving a file that already does as it is.
    Set {
        ending: Ending,
        files: Vec<PathBuf>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Ending {
    Lf,
    Crlf,
}

/// The number of lines ending with LF alone, and with CRLF.
fn count(contents: &[u8]) -> (usize, usize) {
    let mut lf = 0;
    let mut crlf = 0;
    for (index, byte) in contents.iter().enumerate() {
        if *byte == b'\n' {
            match index > 0 && contents[index - 1] == b'\r' {
                true => crlf += 1,
                false => lf += 1,
            }
        }
    }
    (lf, crlf)
}

/// The contents with every line ending as the ending, a `\r` not before a `\n` kept as it is.
fn convert(contents: &[u8], ending: Ending) -> Vec<u8> {
    let mut converted = Vec::with_capacity(contents.len() + contents.len() / 16);
    for (index, byte) in contents.iter().enumerate() {
        match byte {
            b'\r' if contents.get(index + 1) == Some(&b'\n') => {}
            b'\n' => {
                if let Ending::Crlf = ending {
                    converted.push(b'\r');
                }
                converted.push(b'\n');
            }
            byte => converted.push(*byte),
        }
    }
    converted
}

fn main() -> ExitCode {
    let args = Args::parse();
    let mut failed = false;
    match args.command {
        Command::Show {
            files,
        } => {
            for file in files {
                match fs::read(&file) {
                    Ok(contents) => {
                        let (lf, crlf) = count(&contents);
                        println!("lf: {lf}, crlf: {crlf}, file: {}", file.display());
                    }
                    Err(error) => {
                        eprintln!("Unable to read the file. file: {}, cause: {error}", file.display());
                        failed = true;
                    }
                }
            }
        }
        Command::Set {
            ending,
            files,
        } => {
            for file in files {
                let result = fs::read(&file).and_then(|contents| {
                    let converted = convert(&contents, ending);
                    match converted == contents {
                        true => Ok(false),
                        false => fs::write(&file, converted).map(|_| true),
                    }
                });
                match result {
                    Ok(true) => println!("Changed. file: {}", file.display()),
                    Ok(false) => println!("Unchanged. file: {}", file.display()),
                    Err(error) => {
                        eprintln!("Unable to change the file. file: {}, cause: {error}", file.display());
                        failed = true;
                    }
                }
            }
        }
    }
    match failed {
        true => ExitCode::FAILURE,
        false => ExitCode::SUCCESS,
    }
}
