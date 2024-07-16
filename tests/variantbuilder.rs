// Run tests as follows:
// `cargo test --features="cli"`

#[macro_use]
extern crate makerpnp;

#[cfg(feature="cli")]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use assert_cmd::prelude::OutputAssertExt;
    use csv::QuoteStyle;
    use indoc::indoc;
    use predicates::function::FnPredicate;
    use predicates::prelude::*;
    use tempfile::{tempdir, TempDir};

    #[test]
    fn build() -> Result<(), std::io::Error> {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_variantbuilder"));

        // and
        let temp_dir = tempdir()?;

        // and placements
        let (test_placements_path, test_placements_file_name) = build_temp_csv_file(&temp_dir, "placements");

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
            ref_des: "R3".to_string(),
            name: "RES_0402".to_string(),
            value: "470R 1/16W 5%".to_string(),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "R4".to_string(),
            name: "RES_0402".to_string(),
            value: "220R 1/16W 5%".to_string(),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "C1".to_string(),
            name: "CAP_0402".to_string(),
            value: "10uF 6.3V 20%".to_string(),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "J1".to_string(),
            name: "HEADER_2P".to_string(),
            value: "POWER".to_string(),
        })?;

        writer.flush()?;

        // and
        let placements_arg = format!("--placements={}", test_placements_file_name.to_str().unwrap());

        // and substitutions
        let (test_substitutions_path, test_substitutions_file_name) = build_temp_csv_file(&temp_dir, "substitutions");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_substitutions_path)?;

        writer.serialize(TestSubstitutionRecord {
            eda: "DipTrace".to_string(),
            name_pattern: "HEADER_2P".to_string(),
            value_pattern: "BLACK".to_string(),
            name: "CONN_HEADER_2P54_2P_NS_V".to_string(),
            value: "BLACK".to_string(),
        })?;
        writer.serialize(TestSubstitutionRecord {
            eda: "DipTrace".to_string(),
            name_pattern: "HEADER_2P".to_string(),
            value_pattern: "POWER".to_string(),
            name: "HEADER_2P".to_string(),
            value: "BLACK".to_string(),
        })?;

        writer.flush()?;

        // and
        let substitutions_arg = format!("--substitutions={}", test_substitutions_file_name.to_str().unwrap());

        // and load-out
        let (test_load_out_path, test_load_out_file_name) = build_temp_csv_file(&temp_dir, "load_out");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_load_out_path)?;

        writer.serialize(TestLoadOutRecord {
            reference: "FEEDER_1".to_string(),
            manufacturer: "RES_MFR2".to_string(),
            mpn: "RES2".to_string(),
        })?;

        // and two resistors which can both be used by the same placement
        writer.serialize(TestLoadOutRecord {
            reference: "FEEDER_2".to_string(),
            manufacturer: "RES_MFR3".to_string(),
            mpn: "RES3".to_string(),
        })?;
        writer.serialize(TestLoadOutRecord {
            reference: "FEEDER_3".to_string(),
            manufacturer: "RES_MFR4".to_string(),
            mpn: "RES4".to_string(),
        })?;

        writer.flush()?;

        let load_out_arg = format!("--load-out={}", test_load_out_file_name.to_str().unwrap());

        // and parts
        let (test_parts_path, test_parts_file_name) = build_temp_csv_file(&temp_dir, "parts");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_parts_path)?;

        writer.serialize(TestPartRecord {
            manufacturer: "CONN_MFR1".to_string(),
            mpn: "CONN1".to_string(),
        })?;
        writer.serialize(TestPartRecord {
            manufacturer: "RES_MFR1".to_string(),
            mpn: "RES1".to_string(),
        })?;
        writer.serialize(TestPartRecord {
            manufacturer: "RES_MFR2".to_string(),
            mpn: "RES2".to_string(),
        })?;
        writer.serialize(TestPartRecord {
            manufacturer: "RES_MFR3".to_string(),
            mpn: "RES3".to_string(),
        })?;
        writer.serialize(TestPartRecord {
            manufacturer: "RES_MFR4".to_string(),
            mpn: "RES4".to_string(),
        })?;
        writer.serialize(TestPartRecord {
            manufacturer: "RES_MFR5".to_string(),
            mpn: "RES5".to_string(),
        })?;
        writer.serialize(TestPartRecord {
            manufacturer: "RES_MFR6".to_string(),
            mpn: "RES6".to_string(),
        })?;

        writer.flush()?;

        let parts_arg = format!("--parts={}", test_parts_file_name.to_str().unwrap());

        // and part mappings
        let (test_part_mappings_path, test_part_mappings_file_name) = build_temp_csv_file(&temp_dir, "part_mappings");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_part_mappings_path)?;

        // and a mapping for a resistor
        writer.serialize(TestPartMappingRecord {
            eda: "DipTrace".to_string(),
            name: "RES_0402".to_string(),
            value: "330R 1/16W 5%".to_string(),
            // maps to
            manufacturer: "RES_MFR1".to_string(),
            mpn: "RES1".to_string(),
        })?;
        // and an alternative mapping for the same resistor
        // Note: having two potential mappings forces the system (or user) to select one
        writer.serialize(TestPartMappingRecord {
            eda: "DipTrace".to_string(),
            name: "RES_0402".to_string(),
            value: "330R 1/16W 5%".to_string(),
            // maps to
            manufacturer: "RES_MFR2".to_string(),
            mpn: "RES2".to_string(),
        })?;

        // and two more mappings for a different resistor
        writer.serialize(TestPartMappingRecord {
            eda: "DipTrace".to_string(),
            name: "RES_0402".to_string(),
            value: "470R 1/16W 5%".to_string(),
            // maps to
            manufacturer: "RES_MFR3".to_string(),
            mpn: "RES3".to_string(),
        })?;
        writer.serialize(TestPartMappingRecord {
            eda: "DipTrace".to_string(),
            name: "RES_0402".to_string(),
            value: "470R 1/16W 5%".to_string(),
            // maps to
            manufacturer: "RES_MFR4".to_string(),
            mpn: "RES4".to_string(),
        })?;

        // and two more mappings for another different resistor
        writer.serialize(TestPartMappingRecord {
            eda: "DipTrace".to_string(),
            name: "RES_0402".to_string(),
            value: "220R 1/16W 5%".to_string(),
            // maps to
            manufacturer: "RES_MFR5".to_string(),
            mpn: "RES5".to_string(),
        })?;
        writer.serialize(TestPartMappingRecord {
            eda: "DipTrace".to_string(),
            name: "RES_0402".to_string(),
            value: "220R 1/16W 5%".to_string(),
            // maps to
            manufacturer: "RES_MFR6".to_string(),
            mpn: "RES6".to_string(),
        })?;

        // and a single mapping for the connector
        writer.serialize(TestPartMappingRecord {
            eda: "DipTrace".to_string(),
            name: "CONN_HEADER_2P54_2P_NS_V".to_string(),
            value: "BLACK".to_string(),
            // maps to
            manufacturer: "CONN_MFR1".to_string(),
            mpn: "CONN1".to_string(),
        })?;

        writer.flush()?;

        let part_mappings_arg = format!("--part-mappings={}", test_part_mappings_file_name.to_str().unwrap());

        // and
        let expected_part_mapping_tree = indoc! {"
            Mapping Result
            ├── R1 (name: 'RES_0402', value: '330R 1/16W 5%')
            │   ├── manufacturer: 'RES_MFR1', mpn: 'RES1'
            │   └── manufacturer: 'RES_MFR2', mpn: 'RES2' (Found in load-out, reference: 'FEEDER_1')
            ├── R3 (name: 'RES_0402', value: '470R 1/16W 5%')
            │   ├── manufacturer: 'RES_MFR3', mpn: 'RES3' (Found in load-out, reference: 'FEEDER_2')
            │   ├── manufacturer: 'RES_MFR4', mpn: 'RES4' (Found in load-out, reference: 'FEEDER_3')
            │   └── ERROR: Unresolved mapping - Conflicting rules.
            ├── R4 (name: 'RES_0402', value: '220R 1/16W 5%')
            │   ├── manufacturer: 'RES_MFR5', mpn: 'RES5'
            │   ├── manufacturer: 'RES_MFR6', mpn: 'RES6'
            │   └── ERROR: Unresolved mapping - No rules applied.
            ├── C1 (name: 'CAP_0402', value: '10uF 6.3V 20%')
            │   └── ERROR: Unresolved mapping - No mappings found.
            └── J1 (name: 'HEADER_2P', value: 'POWER')
                └── Substituted (name: 'HEADER_2P', value: 'BLACK'), by (name_pattern: 'HEADER_2P', value_pattern: 'POWER')
                    └── Substituted (name: 'CONN_HEADER_2P54_2P_NS_V', value: 'BLACK'), by (name_pattern: 'HEADER_2P', value_pattern: 'BLACK')
                        └── manufacturer: 'CONN_MFR1', mpn: 'CONN1' (Auto-selected)
        "};

        // and
        let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
        let trace_log_arg = format!("--trace={}", test_trace_log_file_name.to_str().unwrap());

        // when
        cmd.args([
            trace_log_arg.as_str(),
            "build",
            placements_arg.as_str(),
            parts_arg.as_str(),
            part_mappings_arg.as_str(),
            load_out_arg.as_str(),
            substitutions_arg.as_str(),
            "--name",
            "Variant 1",
            "--ref-des-list=R1,R3,R4,C1,J1",
        ])
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"))
            .success();

        // and
        let trace_content: String = fs::read_to_string(test_trace_log_path.clone())?;
        println!("{}", trace_content);

        // method 1 (when this fails, you get an error with details, and the stacktrace contains the line number)
        let _remainder = trace_content.clone();
        let _remainder = assert_inorder!(_remainder, "Loaded 6 placements\n");
        let _remainder = assert_inorder!(_remainder, "Loaded 2 substitution rules\n");
        let _remainder = assert_inorder!(_remainder, "Loaded 7 parts\n");
        let _remainder = assert_inorder!(_remainder, "Loaded 7 part mappings\n");
        let _remainder = assert_inorder!(_remainder, "Loaded 3 load-out items\n");
        let _remainder = assert_inorder!(_remainder, "Assembly variant: Variant 1\n");
        let _remainder = assert_inorder!(_remainder, "Ref_des list: R1, R3, R4, C1, J1\n");
        let _remainder = assert_inorder!(_remainder, "Matched 5 placements for assembly variant\n");
        let _remainder = assert_inorder!(_remainder, expected_part_mapping_tree);
        let _remainder = assert_inorder!(_remainder, "Mapping failures\n");

        // method 2 (when this fails, you get an error, with details, but stacktrace does not contain the exact line number)
        assert_contains_inorder!(trace_content, [
            "Loaded 6 placements\n",
            "Loaded 2 substitution rules\n",
            "Loaded 7 parts\n",
            "Loaded 7 part mappings\n",
            "Loaded 3 load-out items\n",
            "Assembly variant: Variant 1\n",
            "Ref_des list: R1, R3, R4, C1, J1\n",
            "Matched 5 placements for assembly variant\n",
            expected_part_mapping_tree,
            "Mapping failures\n",
        ]);

        Ok(())
    }

    fn print(message: &str) -> FnPredicate<fn(&str) -> bool, str> {
        println!("{}:", message);
        predicate::function(|content| {
            println!("{}", content);
            true
        })
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
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff("variantbuilder 0.1.0\n")));
    }

    #[test]
    fn no_args() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_variantbuilder"));

        // and
        let expected_output = indoc! {"
            Usage: variantbuilder [OPTIONS] [COMMAND]

            Commands:
              build  Build variant
              help   Print this message or the help of the given subcommand(s)

            Options:
                  --trace[=<TRACE>]  Trace log file
              -h, --help             Print help
              -V, --version          Print version
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
            .stderr(print("stderr").and(predicate::str::diff(expected_output)))
            .stdout(print("stdout"));
    }

    #[test]
    fn help_for_build_subcommand() {
        // given
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_variantbuilder"));

        // and
        let expected_output = indoc! {"
            Build variant

            Usage: variantbuilder build [OPTIONS] --placements <FILE> --parts <FILE> --part-mappings <FILE> --substitutions <FILE>

            Options:
                  --load-out <FILE>                   Load-out file
                  --placements <FILE>                 Placements file
                  --parts <FILE>                      Parts file
                  --part-mappings <FILE>              Part-mappings file
                  --substitutions <FILE>              Substitutions file
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
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff(expected_output)));
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestDiptracePlacementRecord {
        ref_des: String,
        name: String,
        value: String,
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestPartRecord {
        manufacturer: String,
        mpn: String,
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestLoadOutRecord {
        reference: String,
        manufacturer: String,
        mpn: String,
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestPartMappingRecord {
        eda: String,
        name: String,
        value: String,
        manufacturer: String,
        mpn: String,
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestSubstitutionRecord {
        eda: String,
        name_pattern: String,
        value_pattern: String,
        name: String,
        value: String,
    }

    fn build_temp_csv_file(temp_dir: &TempDir, base: &str) -> (PathBuf, OsString) {
        build_temp_file(temp_dir, base, "csv")
    }

    fn build_temp_file(temp_dir: &TempDir, base: &str, extension: &str) -> (PathBuf, OsString) {
        let mut path_buf = temp_dir.path().to_path_buf();
        path_buf.push(format!("{}.{}", base, extension));

        let absolute_path = path_buf.clone().into_os_string();
        println!("{} file: {}",
                 base.replace('_', " "),
                 absolute_path.to_str().unwrap()
        );

        (path_buf, absolute_path)
    }
}