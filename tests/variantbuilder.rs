// Run tests as follows:
// `cargo test --features="cli"`

#[macro_use]
extern crate makerpnp;

#[cfg(feature="cli")]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::fs::read_to_string;
    use std::path::PathBuf;
    use std::process::Command;
    use assert_cmd::prelude::OutputAssertExt;
    use csv::QuoteStyle;
    use indoc::indoc;
    use predicates::function::FnPredicate;
    use predicates::prelude::*;
    use predicates_tree::CaseTreeExt;

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
            value: "330R".to_string(),
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
            ref_des: "D1".to_string(),
            name: "DIO_0603".to_string(),
            value: "1A 10V".to_string(),
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
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "TP1".to_string(),
            name: "".to_string(),
            value: "".to_string(),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "TP2".to_string(),
            name: "".to_string(),
            value: "".to_string(),
        })?;

        writer.flush()?;

        let placements_arg = format!("--placements={}", test_placements_file_name.to_str().unwrap());

        // and per-assembly-variant substitutions
        let (test_assembly_substitutions_path, test_assembly_substitutions_file_name) = build_temp_csv_file(&temp_dir, "assembly-substitutions");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_assembly_substitutions_path)?;

        writer.serialize(TestDiptraceSubstitutionRecord {
            eda: "DipTrace".to_string(),
            name_pattern: "RES_0402".to_string(),
            value_pattern: "330R".to_string(),
            name: "RES_0402".to_string(),
            value: "330R 1/16W 5%".to_string(),
        })?;

        writer.flush()?;

        // and global substitutions
        let (test_global_substitutions_path, test_global_substitutions_file_name) = build_temp_csv_file(&temp_dir, "global-substitutions");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_global_substitutions_path)?;

        writer.serialize(TestDiptraceSubstitutionRecord {
            eda: "DipTrace".to_string(),
            name_pattern: "HEADER_2P".to_string(),
            value_pattern: "BLACK".to_string(),
            name: "CONN_HEADER_2P54_2P_NS_V".to_string(),
            value: "BLACK".to_string(),
        })?;
        writer.serialize(TestDiptraceSubstitutionRecord {
            eda: "DipTrace".to_string(),
            name_pattern: "HEADER_2P".to_string(),
            value_pattern: "POWER".to_string(),
            name: "HEADER_2P".to_string(),
            value: "BLACK".to_string(),
        })?;

        writer.flush()?;

        let substitutions_arg = format!("--substitutions={},{}",
            test_assembly_substitutions_file_name.to_str().unwrap(),
            test_global_substitutions_file_name.to_str().unwrap(),
        );

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
        writer.serialize(TestPartRecord {
            manufacturer: "DIO_MFR1".to_string(),
            mpn: "DIO1".to_string(),
        })?;
        writer.serialize(TestPartRecord {
            manufacturer: "DIO_MFR2".to_string(),
            mpn: "DIO2".to_string(),
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
            name: Some("RES_0402".to_string()),
            value: Some("330R 1/16W 5%".to_string()),
            // maps to
            manufacturer: "RES_MFR1".to_string(),
            mpn: "RES1".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;
        // and an alternative mapping for the same resistor
        // Note: having two potential mappings forces the system (or user) to select one
        writer.serialize(TestPartMappingRecord {
            name: Some("RES_0402".to_string()),
            value: Some("330R 1/16W 5%".to_string()),
            // maps to
            manufacturer: "RES_MFR2".to_string(),
            mpn: "RES2".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;

        // and two more mappings for a different resistor
        writer.serialize(TestPartMappingRecord {
            name: Some("RES_0402".to_string()),
            value: Some("470R 1/16W 5%".to_string()),
            // maps to
            manufacturer: "RES_MFR3".to_string(),
            mpn: "RES3".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;
        writer.serialize(TestPartMappingRecord {
            name: Some("RES_0402".to_string()),
            value: Some("470R 1/16W 5%".to_string()),
            // maps to
            manufacturer: "RES_MFR4".to_string(),
            mpn: "RES4".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;

        // and two more mappings for another different resistor
        writer.serialize(TestPartMappingRecord {
            name: Some("RES_0402".to_string()),
            value: Some("220R 1/16W 5%".to_string()),
            // maps to
            manufacturer: "RES_MFR5".to_string(),
            mpn: "RES5".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;
        writer.serialize(TestPartMappingRecord {
            name: Some("RES_0402".to_string()),
            value: Some("220R 1/16W 5%".to_string()),
            // maps to
            manufacturer: "RES_MFR6".to_string(),
            mpn: "RES6".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;
        // and two more mappings for a diode
        writer.serialize(TestPartMappingRecord {
            name: Some("DIO_0603".to_string()),
            value: Some("1A 10V".to_string()),
            // maps to
            manufacturer: "DIO_MFR1".to_string(),
            mpn: "DIO1".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;
        writer.serialize(TestPartMappingRecord {
            name: Some("DIO_0603".to_string()),
            value: Some("1A 10V".to_string()),
            // maps to
            manufacturer: "DIO_MFR2".to_string(),
            mpn: "DIO2".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;
        // and a single mapping for the connector
        writer.serialize(TestPartMappingRecord {
            name: Some("CONN_HEADER_2P54_2P_NS_V".to_string()),
            value: Some("BLACK".to_string()),
            // maps to
            manufacturer: "CONN_MFR1".to_string(),
            mpn: "CONN1".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;

        writer.flush()?;

        let part_mappings_arg = format!("--part-mappings={}", test_part_mappings_file_name.to_str().unwrap());

        // and assembly-rules
        let (test_assembly_rule_path, test_assembly_rule_file_name) = build_temp_csv_file(&temp_dir, "assembly_rule");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_assembly_rule_path)?;

        writer.serialize(TestAssemblyRuleRecord {
            ref_des: "D1".to_string(),
            manufacturer: "DIO_MFR2".to_string(),
            mpn: "DIO2".to_string(),
        })?;

        writer.flush()?;

        let assembly_rules_arg = format!("--assembly-rules={}", test_assembly_rule_file_name.to_str().unwrap());

        // and
        let expected_part_mapping_tree = indoc! {"
            Mapping Result
            ├── R1 (name: 'RES_0402', value: '330R')
            │   └── Substituted (name: 'RES_0402', value: '330R 1/16W 5%'), by (name_pattern: 'RES_0402', value_pattern: '330R')
            │       ├── manufacturer: 'RES_MFR1', mpn: 'RES1'
            │       └── manufacturer: 'RES_MFR2', mpn: 'RES2' (Found in load-out, reference: 'FEEDER_1')
            ├── R3 (name: 'RES_0402', value: '470R 1/16W 5%')
            │   ├── manufacturer: 'RES_MFR3', mpn: 'RES3' (Found in load-out, reference: 'FEEDER_2')
            │   ├── manufacturer: 'RES_MFR4', mpn: 'RES4' (Found in load-out, reference: 'FEEDER_3')
            │   └── ERROR: Unresolved mapping - Conflicting rules.
            ├── R4 (name: 'RES_0402', value: '220R 1/16W 5%')
            │   ├── manufacturer: 'RES_MFR5', mpn: 'RES5'
            │   ├── manufacturer: 'RES_MFR6', mpn: 'RES6'
            │   └── ERROR: Unresolved mapping - No rules applied.
            ├── D1 (name: 'DIO_0603', value: '1A 10V')
            │   ├── manufacturer: 'DIO_MFR1', mpn: 'DIO1'
            │   └── manufacturer: 'DIO_MFR2', mpn: 'DIO2' (Matched assembly-rule)
            ├── C1 (name: 'CAP_0402', value: '10uF 6.3V 20%')
            │   └── ERROR: Unresolved mapping - No mappings found.
            ├── J1 (name: 'HEADER_2P', value: 'POWER')
            │   └── Substituted (name: 'HEADER_2P', value: 'BLACK'), by (name_pattern: 'HEADER_2P', value_pattern: 'POWER')
            │       └── Substituted (name: 'CONN_HEADER_2P54_2P_NS_V', value: 'BLACK'), by (name_pattern: 'HEADER_2P', value_pattern: 'BLACK')
            │           └── manufacturer: 'CONN_MFR1', mpn: 'CONN1' (Auto-selected)
            ├── TP1 (name: '', value: '')
            │   └── ERROR: Unresolved mapping - No mappings found.
            └── TP2 (name: '', value: '')
                └── ERROR: Unresolved mapping - No mappings found.
        "};

        // and
        let expected_csv_content = indoc! {"
            \"RefDes\",\"Manufacturer\",\"Mpn\",\"Place\"
            \"R1\",\"RES_MFR2\",\"RES2\",\"true\"
            \"R3\",\"\",\"\",\"true\"
            \"R4\",\"\",\"\",\"true\"
            \"D1\",\"DIO_MFR2\",\"DIO2\",\"true\"
            \"C1\",\"\",\"\",\"true\"
            \"J1\",\"CONN_MFR1\",\"CONN1\",\"true\"
            \"TP1\",\"\",\"\",\"false\"
            \"TP2\",\"\",\"\",\"false\"
        "};

        let (test_csv_output_path, test_csv_output_file_name) = build_temp_csv_file(&temp_dir, "output");
        let csv_output_arg = format!("--output={}", test_csv_output_file_name.to_str().unwrap());

        // and
        let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
        let trace_log_arg = format!("--trace={}", test_trace_log_file_name.to_str().unwrap());

        // when
        cmd.args([
            trace_log_arg.as_str(),
            "build",
            "--eda=diptrace",
            placements_arg.as_str(),
            parts_arg.as_str(),
            part_mappings_arg.as_str(),
            load_out_arg.as_str(),
            substitutions_arg.as_str(),
            assembly_rules_arg.as_str(),
            csv_output_arg.as_str(),
            "--name",
            "Variant 1",
            "--ref-des-list=R1,R3,R4,D1,C1,J1,TP1,TP2",
            "--ref-des-disable-list=TP1,TP2",
        ])
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"))
            .success();

        // and
        let trace_content: String = fs::read_to_string(test_trace_log_path.clone())?;
        println!("{}", trace_content);

        // and
        let expected_substitutions_file_1_message = format!("Loaded 1 substitution rules from {}\n", test_assembly_substitutions_file_name.to_str().unwrap());
        let expected_substitutions_file_2_message = format!("Loaded 2 substitution rules from {}\n", test_global_substitutions_file_name.to_str().unwrap());

        // method 1 (when this fails, you get an error with details, and the stacktrace contains the line number)
        let _remainder = trace_content.clone();
        let _remainder = assert_inorder!(_remainder, "Loaded 9 placements\n");
        let _remainder = assert_inorder!(_remainder, expected_substitutions_file_1_message.as_str());
        let _remainder = assert_inorder!(_remainder, expected_substitutions_file_2_message.as_str());
        let _remainder = assert_inorder!(_remainder, "Loaded 9 parts\n");
        let _remainder = assert_inorder!(_remainder, "Loaded 9 part mappings\n");
        let _remainder = assert_inorder!(_remainder, "Loaded 3 load-out items\n");
        let _remainder = assert_inorder!(_remainder, "Loaded 1 assembly rules\n");
        let _remainder = assert_inorder!(_remainder, "Assembly variant: Variant 1\n");
        let _remainder = assert_inorder!(_remainder, "Ref_des list: R1, R3, R4, D1, C1, J1, TP1, TP2\n");
        let _remainder = assert_inorder!(_remainder, "Matched 8 placements for assembly variant\n");
        let _remainder = assert_inorder!(_remainder, expected_part_mapping_tree);
        let _remainder = assert_inorder!(_remainder, "Mapping failures\n");

        // method 2 (when this fails, you get an error, with details, but stacktrace does not contain the exact line number)
        assert_contains_inorder!(trace_content, [
            "Loaded 9 placements\n",
            expected_substitutions_file_1_message.as_str(),
            "Loaded 9 parts\n",
            "Loaded 9 part mappings\n",
            "Loaded 3 load-out items\n",
            "Loaded 1 assembly rules\n",
            "Assembly variant: Variant 1\n",
            "Ref_des list: R1, R3, R4, D1, C1, J1, TP1, TP2\n",
            "Matched 8 placements for assembly variant\n",
            expected_part_mapping_tree,
            "Mapping failures\n",
        ]);

        // and
        let csv_output_file = assert_fs::NamedTempFile::new(test_csv_output_path).unwrap();
        let csv_content = read_to_string(csv_output_file)?;
        println!("{}", csv_content);

        // TODO improve readability of this assertion, use a macro?
        if let Some(case) = predicate::str::diff(expected_csv_content).find_case(false, csv_content.as_str()) {
            panic!("Unexpected CSV content\n{}", case.tree());
        }

        Ok(())
    }

    #[test]
    fn build_kicad() -> Result<(), std::io::Error> {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_variantbuilder"));

        // and
        let temp_dir = tempdir()?;

        // and placements
        let (test_placements_path, test_placements_file_name) = build_temp_csv_file(&temp_dir, "placements-all-pos");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_placements_path)?;

        writer.serialize(TestKiCadPlacementRecord {
            ref_des: "R1".to_string(),
            package: "R_0402_1005Metric".to_string(),
            val: "330R".to_string(),
        })?;

        writer.flush()?;

        let placements_arg = format!("--placements={}", test_placements_file_name.to_str().unwrap());

        // and global substitutions
        let (test_global_substitutions_path, test_global_substitutions_file_name) = build_temp_csv_file(&temp_dir, "global-substitutions");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_global_substitutions_path)?;

        writer.serialize(TestKiCadSubstitutionRecord {
            eda: "KiCad".to_string(),
            package_pattern: "R_0402_1005Metric".to_string(),
            val_pattern: "330R".to_string(),
            package: "R_0402_1005Metric".to_string(),
            val: "330R 1/16W 5%".to_string(),
        })?;

        writer.flush()?;

        let substitutions_arg = format!("--substitutions={}",
            test_global_substitutions_file_name.to_str().unwrap(),
        );

        // and parts
        let (test_parts_path, test_parts_file_name) = build_temp_csv_file(&temp_dir, "parts");

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_parts_path)?;

        writer.serialize(TestPartRecord {
            manufacturer: "RES_MFR1".to_string(),
            mpn: "RES1".to_string(),
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
            package: Some("R_0402_1005Metric".to_string()),
            val: Some("330R 1/16W 5%".to_string()),
            // maps to
            manufacturer: "RES_MFR1".to_string(),
            mpn: "RES1".to_string(),
            ..TestPartMappingRecord::kicad_defaults()
        })?;

        writer.flush()?;

        let part_mappings_arg = format!("--part-mappings={}", test_part_mappings_file_name.to_str().unwrap());

        let (test_csv_output_path, test_csv_output_file_name) = build_temp_csv_file(&temp_dir, "output");
        let csv_output_arg = format!("--output={}", test_csv_output_file_name.to_str().unwrap());

        // and
        let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
        let trace_log_arg = format!("--trace={}", test_trace_log_file_name.to_str().unwrap());

        // and
        let expected_part_mapping_tree = indoc! {"
            Mapping Result
            └── R1 (package: 'R_0402_1005Metric', val: '330R')
                └── Substituted (package: 'R_0402_1005Metric', val: '330R 1/16W 5%'), by (package_pattern: 'R_0402_1005Metric', val_pattern: '330R')
                    └── manufacturer: 'RES_MFR1', mpn: 'RES1' (Auto-selected)
        "};

        // and
        let expected_csv_content = indoc! {"
            \"RefDes\",\"Manufacturer\",\"Mpn\",\"Place\"
            \"R1\",\"RES_MFR1\",\"RES1\",\"true\"
        "};

        // when
        cmd.args([
            trace_log_arg.as_str(),
            "build",
            "--eda=kicad",
            placements_arg.as_str(),
            parts_arg.as_str(),
            part_mappings_arg.as_str(),
            csv_output_arg.as_str(),
            substitutions_arg.as_str(),
            "--name",
            "Variant 1",
            "--ref-des-list=R1",
        ])
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"))
            .success();

        // and
        let trace_content: String = fs::read_to_string(test_trace_log_path.clone())?;
        println!("{}", trace_content);

        let expected_substitutions_file_1_message = format!("Loaded 1 substitution rules from {}\n", test_global_substitutions_file_name.to_str().unwrap());

        let _remainder = trace_content.clone();
        let _remainder = assert_inorder!(_remainder, "Loaded 1 placements\n");
        let _remainder = assert_inorder!(_remainder, expected_substitutions_file_1_message.as_str());
        let _remainder = assert_inorder!(_remainder, "Loaded 1 parts\n");
        let _remainder = assert_inorder!(_remainder, "Assembly variant: Variant 1\n");
        let _remainder = assert_inorder!(_remainder, "Ref_des list: R1\n");
        let _remainder = assert_inorder!(_remainder, "Matched 1 placements for assembly variant\n");
        let _remainder = assert_inorder!(_remainder, expected_part_mapping_tree);

        // and
        let csv_output_file = assert_fs::NamedTempFile::new(test_csv_output_path).unwrap();
        let csv_content = read_to_string(csv_output_file)?;
        println!("{}", csv_content);

        // TODO improve readability of this assertion, use a macro?
        if let Some(case) = predicate::str::diff(expected_csv_content).find_case(false, csv_content.as_str()) {
            panic!("Unexpected CSV content\n{}", case.tree());
        }

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

            Usage: variantbuilder build [OPTIONS] --eda <EDA> --placements <FILE> --parts <FILE> --part-mappings <FILE> --output <FILE>

            Options:
                  --eda <EDA>
                      EDA tool [possible values: diptrace, kicad]
                  --load-out <FILE>
                      Load-out file
                  --placements <FILE>
                      Placements file
                  --parts <FILE>
                      Parts file
                  --part-mappings <FILE>
                      Part-mappings file
                  --substitutions[=<FILE>...]
                      Substitutions files
                  --ref-des-disable-list [<REF_DES_DISABLE_LIST>...]
                      List of reference designators to disable (use for do-not-fit, no-place, test-points, fiducials, etc)
                  --assembly-rules <FILE>
                      Assembly rules file
                  --output <FILE>
                      Output file
                  --name <NAME>
                      Name of assembly variant [default: Default]
                  --ref-des-list [<REF_DES_LIST>...]
                      List of reference designators
              -h, --help
                      Print help
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
    struct TestKiCadPlacementRecord {
        #[serde(rename(serialize = "ref"))]
        ref_des: String,
        package: String,
        val: String,
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
    struct TestAssemblyRuleRecord {
        ref_des: String,
        manufacturer: String,
        mpn: String,
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestPartMappingRecord {
        //
        // From
        //

        eda: String,

        // DipTrace specific
        name: Option<String>,
        value: Option<String>,

        // KiCad specific
        package: Option<String>,
        val: Option<String>,

        //
        // To
        //
        manufacturer: String,
        mpn: String,
    }

    impl TestPartMappingRecord {
        pub fn diptrace_defaults() -> TestPartMappingRecord {
            TestPartMappingRecord {
                eda: "DipTrace".to_string(),
                ..Default::default()
            }
        }

        pub fn kicad_defaults() -> TestPartMappingRecord {
            TestPartMappingRecord {
                eda: "KiCad".to_string(),
                ..Default::default()
            }
        }
    }

    impl Default for TestPartMappingRecord {
        fn default() -> Self {
            Self {
                eda: "".to_string(),
                name: None,
                value: None,
                package: None,
                val: None,
                manufacturer: "".to_string(),
                mpn: "".to_string(),
            }
        }
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestDiptraceSubstitutionRecord {
        eda: String,
        name_pattern: String,
        value_pattern: String,
        name: String,
        value: String,
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestKiCadSubstitutionRecord {
        eda: String,
        package_pattern: String,
        val_pattern: String,
        package: String,
        val: String,
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