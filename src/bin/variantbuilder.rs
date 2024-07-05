use std::path::PathBuf;
use anyhow::{bail, Error};
use clap::{Args, Parser, Subcommand};
use thiserror::Error;
use makerpnp::assembly::{AssemblyVariant, AssemblyVariantProcessor, Placement};

#[derive(Parser)]
struct Opts {
    /// Show version information
    #[arg(short = 'V', long)]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Args)]
#[derive(Clone)]
struct AssemblyVariantArgs {
    /// Name of assembly variant
    #[arg(long, default_value = "Default")]
    name: String,

    /// List of reference designators
    #[arg(long, num_args = 0.., value_delimiter = ',')]
    ref_des_list: Vec<String>
}

#[allow(dead_code)]
#[derive(Error, Debug)]
enum AssemblyVariantError {
    #[error("Unknown error")]
    Unknown
}

impl AssemblyVariantArgs {
    pub fn build_assembly_variant(&self) -> Result<AssemblyVariant, AssemblyVariantError> {
        // TODO add all placements to the refdes list if the ref_des_list is empty
        Ok(AssemblyVariant::new(
            self.name.clone(),
            self.ref_des_list.clone(),
        ))
    }
}

#[derive(Subcommand)]
#[command(arg_required_else_help(true))]
enum Commands {
    /// Build variant
    Build {
        /// Placements file
        #[arg(short = 'p', long, value_name = "FILE")]
        placements: String,

        #[command(flatten)]
        assembly_variant: AssemblyVariantArgs
    },
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
struct DiptracePlacementRecord {
    ref_des: String,
    name: String,
    value: String,
}

impl DiptracePlacementRecord {
    pub fn build_placement(&self) -> Result<Placement, ()> {
        Ok(Placement {
            ref_des: self.ref_des.clone(),
        })
    }
}

fn main() -> anyhow::Result<()>{
    let opts = Opts::parse();

    if opts.version {
        return print_version();
    }

    match &opts.command.unwrap() {
        Commands::Build { placements, assembly_variant } => {
            build_assembly_variant(placements, assembly_variant)?;
        },
    }

    Ok(())
}

fn build_assembly_variant(placements: &String, assembly_variant_args: &AssemblyVariantArgs) -> Result<(), Error> {
    let placements_path_buf = PathBuf::from(placements);
    let placements_path = placements_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(placements_path)?;

    let mut placements: Vec<Placement> = vec![];

    for result in csv_reader.deserialize() {
        let record: DiptracePlacementRecord = result?;
        // TODO output the record in verbose mode
        //println!("{:?}", record);

        if let Ok(placement) = record.build_placement() {
            placements.push(placement);
        } else {
            bail!("todo")
        }
    }

    let assembly_variant = assembly_variant_args.build_assembly_variant()?;

    println!("Loaded {} placements", placements.len());
    println!("Assembly variant: {}", assembly_variant.name);
    println!("Ref_des list: {}", assembly_variant.ref_des_list.join(", "));

    let assembly_variant_processor = AssemblyVariantProcessor::default();

    // when
    let result = assembly_variant_processor.process(placements, assembly_variant)?;

    println!("Matched {} placements", result.placements.len());

    Ok(())
}

fn print_version() -> anyhow::Result<()> {
    println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use assert_cmd::prelude::{CommandCargoExt, OutputAssertExt};
    use csv::QuoteStyle;
    use indoc::indoc;
    use predicates::prelude::*;
    use tempfile::tempdir;

    #[test]
    fn build() -> Result<(), std::io::Error> {
        // given
        let mut cmd = Command::cargo_bin(env!("CARGO_BIN_NAME"))
            .unwrap();

        // and
        let temp_dir = tempdir()?;

        // and
        let mut test_placements_path = temp_dir.into_path();
        test_placements_path.push("placements.csv");

        let test_placements_file_name = test_placements_path.clone().into_os_string();
        println!("placements file: {}", test_placements_file_name.to_str().unwrap());

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_placements_path)?;

        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "R1".to_string(),
            name: "RES_0402".to_string(),
            value: "330R 1/16W 5%".to_string(),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "R2".to_string(),
            name: "RES_0402".to_string(),
            value: "330R 1/16W 5%".to_string(),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "J1".to_string(),
            name: "CONN_HEADER_2P54_2P_NS_V".to_string(),
            value: "POWER".to_string(),
        })?;

        writer.flush()?;

        // and
        let placements_arg = format!("--placements={}", test_placements_file_name.to_str().unwrap());

        // when
        cmd.args([
            "build",
            placements_arg.as_str(),
            "--name",
            "Variant 1",
            "--ref-des-list=R1,J1"
        ])
            // then
            .assert()
            .success()
            .stdout(
                predicate::str::contains("Loaded 3 placements\n")
                    .and(predicate::str::contains("Assembly variant: Variant 1\n"))
                    .and(predicate::str::contains("Ref_des list: R1, J1\n"))
                    .and(predicate::str::contains("Matched 2 placements\n"))
            );

        Ok(())
    }

    #[test]
    fn version() {
        // given
        let mut cmd = Command::cargo_bin(env!("CARGO_BIN_NAME"))
            .unwrap();

        // when
        cmd.args(["-V"])
            // then
            .assert()
            .success()
            .stdout(predicate::str::diff("variantbuilder 0.1.0\n"));
    }

    #[test]
    fn no_args() {
        // given
        let mut cmd = Command::cargo_bin(env!("CARGO_BIN_NAME"))
            .unwrap();

        // and
        let expected_output = indoc! {"
            Usage: variantbuilder.exe [COMMAND]

            Commands:
              build  Build variant
              help   Print this message or the help of the given subcommand(s)

            Options:
              -V, --version  Show version information
              -h, --help     Print help
        "};

        // TODO report issues with predicate::str::diff and clap
        //      * diff - if the only difference is trailing whitespace, diff fails, without showing a difference.
        //               steps to repeat: place closing quote for `indoc!` call on last line, instead of on a new line.
        //      * clap - if the argument has no comment, the argument details are display and trailing whitespace follows it,
        //               however, the trailing whitespace should not be in the output.
        //               steps to repeat: remove comment on `Opts::version`, run test.

        // when
        cmd
            // then
            .assert()
            .failure()
            .stderr(predicate::str::diff(expected_output));
    }

    #[test]
    fn help_for_build_subcommand() {
        // given
        let mut cmd = Command::cargo_bin(env!("CARGO_BIN_NAME"))
            .unwrap();

        // and
        let expected_output = indoc! {"
            Build variant

            Usage: variantbuilder.exe build [OPTIONS] --placements <FILE>

            Options:
              -p, --placements <FILE>                 Placements file
                  --name <NAME>                       Name of assembly variant [default: Default]
                  --ref-des-list [<REF_DES_LIST>...]  List of reference designators
              -h, --help                              Print help
        "};

        // TODO report issues with clap
        //      * clap - unable to find clap derive documentation for `value_delimiter`
        //               a big gotcha was that it's not possible to use a STRING for the delimiter and
        //               only a CHARACTER is acceptable, i.e. use single quotes around a character.
        //               the error was: '^^^ the trait `IntoResettable<char>` is not implemented for `&str`'

        // when
        cmd.args(["build", "--help"])
            // then
            .assert()
            .success()
            .stdout(predicate::str::diff(expected_output));
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestDiptracePlacementRecord {
        ref_des: String,
        name: String,
        value: String,
    }
}
