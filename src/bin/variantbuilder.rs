use clap::Parser;

#[derive(Parser)]
struct Opts {
    #[arg(short = 'V', long)]
    version: bool,
}

fn main() -> Result<(), anyhow::Error>{
    let opts = Opts::parse();

    if opts.version {
        return print_version();
    }

    todo!()
}

fn print_version() -> anyhow::Result<()> {
    println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use assert_cmd::prelude::{CommandCargoExt, OutputAssertExt};
    use predicates::prelude::*;
    #[test]
    fn version() {
        let mut cmd = Command::cargo_bin(env!("CARGO_BIN_NAME"))
            .unwrap();

        cmd.args(["-V"])
            .assert()
            .stdout(predicate::str::diff("variantbuilder 0.1.0\n"));
    }
}
