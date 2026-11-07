#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
---
//! Replaces a range of a text file's lines with another file's lines, or with none, keeping the file's line endings, to
//! remove or replace a large block of code in one step, where an editor's find and replace would need the whole block.
//!
//! The lines inserted are given the file's line endings, CRLF if most of its lines end with CRLF, else LF.
//!
//! ```text
//! cargo +nightly -Zscript support/scripts/splice_lines.rs show planning/stores_app/tests/tables.rs --from 456 --to 460
//! cargo +nightly -Zscript support/scripts/splice_lines.rs splice planning/stores_app/tests/tables.rs --from 456 --to 888
//! cargo +nightly -Zscript support/scripts/splice_lines.rs splice planning/stores_app/tests/tables.rs --from 456 --to 888 --with new.rs
//! ```

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Replaces a range of a text file's lines, keeping its line endings")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Prints the lines, numbered, to check the range before splicing it.
    Show {
        file: PathBuf,
        /// The first line, counted from 1.
        #[arg(long)]
        from: usize,
        /// The last line, included.
        #[arg(long)]
        to: usize,
    },
    /// Replaces the lines with those of `--with`, or removes them if it's not given.
    Splice {
        file: PathBuf,
        /// The first line, counted from 1.
        #[arg(long)]
        from: usize,
        /// The last line, included.
        #[arg(long)]
        to: usize,
        /// The file whose lines replace them.
        #[arg(long)]
        with: Option<PathBuf>,
    },
}

/// The text's lines, each with its ending, if it has one.
fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// Checks the range is within the file's lines, saying why not.
fn check(from: usize, to: usize, count: usize) -> Result<(), String> {
    match from >= 1 && from <= to && to <= count {
        true => Ok(()),
        false => Err(format!(
            "The range isn't within the file's lines. from: {from}, to: {to}, line_count: {count}"
        )),
    }
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Show {
            file,
            from,
            to,
        } => {
            let text = fs::read_to_string(&file).map_err(|error| format!("Unable to read the file. cause: {error}"))?;
            let lines = lines(&text);
            check(from, to, lines.len())?;
            for (index, line) in lines[from - 1..to].iter().enumerate() {
                print!("{:>6}  {}", from + index, line.trim_end_matches(['\r', '\n']));
                println!();
            }
            Ok(())
        }
        Command::Splice {
            file,
            from,
            to,
            with,
        } => {
            let text = fs::read_to_string(&file).map_err(|error| format!("Unable to read the file. cause: {error}"))?;
            let lines = lines(&text);
            check(from, to, lines.len())?;
            let crlf = lines
                .iter()
                .filter(|line| line.ends_with("\r\n"))
                .count();
            let ending = match crlf * 2 > lines.len() {
                true => "\r\n",
                false => "\n",
            };

            let inserted = match with {
                Some(with) => fs::read_to_string(&with)
                    .map_err(|error| format!("Unable to read the lines to insert. cause: {error}"))?,
                None => String::new(),
            };
            let mut spliced = String::with_capacity(text.len() + inserted.len());
            for line in &lines[..from - 1] {
                spliced.push_str(line);
            }
            for line in inserted.lines() {
                spliced.push_str(line);
                spliced.push_str(ending);
            }
            for line in &lines[to..] {
                spliced.push_str(line);
            }

            fs::write(&file, spliced).map_err(|error| format!("Unable to write the file. cause: {error}"))?;
            println!(
                "Spliced. file: {}, removed: {}, inserted: {}",
                file.display(),
                to + 1 - from,
                inserted.lines().count()
            );
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    match run(Args::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
