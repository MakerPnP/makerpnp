// Run tests as follows:
// `cargo test --features="cli"`

#[cfg(feature="cli")]
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
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_variantbuilder"));

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
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_variantbuilder"));

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
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_variantbuilder"));

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
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_variantbuilder"));

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