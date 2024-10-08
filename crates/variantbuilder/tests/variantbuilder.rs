#[macro_use]
extern crate util;

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;
    use std::process::Command;
    use assert_cmd::prelude::OutputAssertExt;
    use csv::QuoteStyle;
    use indoc::indoc;
    use predicates::prelude::*;
    use predicates_tree::CaseTreeExt;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use tempfile::tempdir;
    use stores::part_mappings::test::TestPartMappingRecord;
    use util::test::{build_temp_csv_file, build_temp_file, prepare_args, print};
    use stores::test::load_out_builder::TestLoadOutRecord;

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
            side: "Top".to_string(),
            x: Decimal::from(10),
            y: Decimal::from(110),
            rotation: Decimal::from(0),

        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "R2".to_string(),
            name: "RES_0402".to_string(),
            value: "330R 1/16W 5%".to_string(),
            side: "Top".to_string(),
            x: Decimal::from(20),
            y: Decimal::from(120),
            rotation: Decimal::from(90),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "R3".to_string(),
            name: "RES_0402".to_string(),
            value: "470R 1/16W 5%".to_string(),
            side: "Top".to_string(),
            x: Decimal::from(30),
            y: Decimal::from(130),
            rotation: Decimal::from(180),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "R4".to_string(),
            name: "RES_0402".to_string(),
            value: "220R 1/16W 5%".to_string(),
            side: "Top".to_string(),
            x: Decimal::from(40),
            y: Decimal::from(140),
            rotation: Decimal::from(270),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "D1".to_string(),
            name: "DIO_0603".to_string(),
            value: "1A 10V".to_string(),
            side: "Top".to_string(),
            x: Decimal::from(50),
            y: Decimal::from(150),
            rotation: Decimal::from(45),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "C1".to_string(),
            name: "CAP_0402".to_string(),
            value: "10uF 6.3V 20%".to_string(),
            side: "Top".to_string(),
            x: Decimal::from(60),
            y: Decimal::from(160),
            rotation: Decimal::from(135),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "J1".to_string(),
            name: "HEADER_2P".to_string(),
            value: "POWER".to_string(),
            side: "Top".to_string(),
            x: Decimal::from(70),
            y: Decimal::from(170),
            rotation: Decimal::from(225),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "TP1".to_string(),
            name: "".to_string(),
            value: "".to_string(),
            side: "Top".to_string(),
            x: Decimal::from(80),
            y: Decimal::from(180),
            rotation: Decimal::from(315),
        })?;
        writer.serialize(TestDiptracePlacementRecord {
            ref_des: "TP2".to_string(),
            name: "".to_string(),
            value: "".to_string(),
            side: "Top".to_string(),
            x: Decimal::from(90),
            y: Decimal::from(190),
            rotation: Decimal::from(5),
        })?;

        writer.flush()?;

        let placements_arg = format!("--placements {}", test_placements_file_name.to_str().unwrap());

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

        let substitutions_arg = format!("--substitutions {},{}",
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

        let load_out_arg = format!("--load-out {}", test_load_out_file_name.to_str().unwrap());

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

        let parts_arg = format!("--parts {}", test_parts_file_name.to_str().unwrap());

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

        let part_mappings_arg = format!("--part-mappings {}", test_part_mappings_file_name.to_str().unwrap());

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

        let assembly_rules_arg = format!("--assembly-rules {}", test_assembly_rule_file_name.to_str().unwrap());

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
        let expected_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
            "R1","RES_MFR2","RES2","true","Top","10","110","0"
            "R3","","","true","Top","30","130","180"
            "R4","","","true","Top","40","140","-90"
            "D1","DIO_MFR2","DIO2","true","Top","50","150","45"
            "C1","","","true","Top","60","160","135"
            "J1","CONN_MFR1","CONN1","true","Top","70","170","-135"
            "TP1","","","false","Top","80","180","-45"
            "TP2","","","false","Top","90","190","5"
        "#}.to_string();

        let (test_csv_output_path, test_csv_output_file_name) = build_temp_csv_file(&temp_dir, "output");
        let csv_output_arg = format!("--output {}", test_csv_output_file_name.to_str().unwrap());

        // and
        let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
        let trace_log_arg = format!("--trace {}", test_trace_log_file_name.to_str().unwrap());

        // when
        cmd.args(prepare_args(vec![
            trace_log_arg.as_str(),
            "build",
            "--eda diptrace",
            placements_arg.as_str(),
            parts_arg.as_str(),
            part_mappings_arg.as_str(),
            load_out_arg.as_str(),
            substitutions_arg.as_str(),
            assembly_rules_arg.as_str(),
            csv_output_arg.as_str(),
            "--name",
            "Variant_1",
            "--ref-des-list R1,R3,R4,D1,C1,J1,TP1,TP2",
            "--ref-des-disable-list TP1,TP2",
        ]))
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"))
            .success();

        // and
        let trace_content: String = read_to_string(test_trace_log_path.clone())?;
        println!("{}", trace_content);

        // and
        let expected_substitutions_file_1_message = format!("Loaded 1 substitution rules from {}\n", test_assembly_substitutions_file_name.to_str().unwrap());
        let expected_substitutions_file_2_message = format!("Loaded 2 substitution rules from {}\n", test_global_substitutions_file_name.to_str().unwrap());

        assert_contains_inorder!(trace_content, [
            "Loaded 9 placements\n",
            expected_substitutions_file_1_message.as_str(),
            expected_substitutions_file_2_message.as_str(),
            "Loaded 9 parts\n",
            "Loaded 9 part mappings\n",
            "Loaded 3 load-out items\n",
            "Loaded 1 assembly rules\n",
            "Assembly variant: Variant_1\n",
            "Ref_des list: R1, R3, R4, D1, C1, J1, TP1, TP2\n",
            "Matched 8 placements for assembly variant\n",
            expected_part_mapping_tree,
            "Mapping failures\n",
        ]);

        // and
        let csv_output_file = assert_fs::NamedTempFile::new(test_csv_output_path).unwrap();
        let csv_content = read_to_string(csv_output_file)?;
        println!("{}", csv_content);

        assert_csv_content(csv_content, expected_csv_content);

        Ok(())
    }

    fn assert_csv_content(csv_content: String, expected_csv_content: String) {
        if let Some(case) = predicate::str::diff(expected_csv_content).find_case(false, csv_content.as_str()) {
            panic!("Unexpected CSV content\n{}", case.tree());
        }
    }

    #[test]
    fn build_kicad_using_default_assembly_variant() -> Result<(), std::io::Error> {
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
            side: "top".to_string(),
            x: Decimal::from(10),
            y: Decimal::from(110),
            rotation: dec!(-179.999),

        })?;

        writer.flush()?;

        let placements_arg = format!("--placements {}", test_placements_file_name.to_str().unwrap());

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

        let substitutions_arg = format!("--substitutions {}",
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

        let parts_arg = format!("--parts {}", test_parts_file_name.to_str().unwrap());

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

        let part_mappings_arg = format!("--part-mappings {}", test_part_mappings_file_name.to_str().unwrap());

        let (test_csv_output_path, test_csv_output_file_name) = build_temp_csv_file(&temp_dir, "output");
        let csv_output_arg = format!("--output {}", test_csv_output_file_name.to_str().unwrap());

        // and
        let (test_trace_log_path, test_trace_log_file_name) = build_temp_file(&temp_dir, "trace", "log");
        let trace_log_arg = format!("--trace {}", test_trace_log_file_name.to_str().unwrap());

        // and
        let expected_part_mapping_tree = indoc! {"
            Mapping Result
            └── R1 (package: 'R_0402_1005Metric', val: '330R')
                └── Substituted (package: 'R_0402_1005Metric', val: '330R 1/16W 5%'), by (package_pattern: 'R_0402_1005Metric', val_pattern: '330R')
                    └── manufacturer: 'RES_MFR1', mpn: 'RES1' (Auto-selected)
        "};

        // and
        let expected_csv_content = indoc! {r#"
            "RefDes","Manufacturer","Mpn","Place","PcbSide","X","Y","Rotation"
            "R1","RES_MFR1","RES1","true","Top","10","110","-179.999"
        "#}.to_string();

        // when
        cmd.args(prepare_args(vec![
            trace_log_arg.as_str(),
            "build",
            "--eda kicad",
            placements_arg.as_str(),
            parts_arg.as_str(),
            part_mappings_arg.as_str(),
            csv_output_arg.as_str(),
            substitutions_arg.as_str(),
        ]))
            // then
            .assert()
            .stderr(print("stderr"))
            .stdout(print("stdout"))
            .success();

        // and
        let trace_content: String = read_to_string(test_trace_log_path.clone())?;
        println!("{}", trace_content);

        let expected_substitutions_file_1_message = format!("Loaded 1 substitution rules from {}\n", test_global_substitutions_file_name.to_str().unwrap());

        assert_contains_inorder!(trace_content, [
            "Loaded 1 placements\n",
            expected_substitutions_file_1_message.as_str(),
            "Loaded 1 parts\n",
            "Assembly variant: Default\n",
            "Ref_des list: \n",
            "Matched 1 placements for assembly variant\n",
            expected_part_mapping_tree,
        ]);

        // and
        let csv_output_file = assert_fs::NamedTempFile::new(test_csv_output_path).unwrap();
        let csv_content = read_to_string(csv_output_file)?;
        println!("{}", csv_content);

        assert_csv_content(csv_content, expected_csv_content);

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
            .stderr(print("stderr"))
            .stdout(print("stdout").and(predicate::str::diff("variantbuilder 0.1.0\n")));
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestDiptracePlacementRecord {
        ref_des: String,
        name: String,
        value: String,
        side: String,
        x: Decimal,
        y: Decimal,
        /// Positive values indicate anti-clockwise rotation
        /// Range is 0 - < 360
        /// Rounding occurs on the 3rd decimal, e.g. 359.991 rounds to 359.99, 359.995 rounds to 360, then gets converted to 0. 
        rotation: Decimal,
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestKiCadPlacementRecord {
        #[serde(rename(serialize = "ref"))]
        ref_des: String,
        package: String,
        val: String,
        side: String,
        x: Decimal,
        y: Decimal,
        /// Positive values indicate anti-clockwise rotation
        /// Range is >-180 to +180.
        /// No rounding.
        /// Values are truncated to 3 decimal places in the UI.
        rotation: Decimal,
    }

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    struct TestPartRecord {
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
}

mod help {
    use std::process::Command;
    use assert_cmd::prelude::OutputAssertExt;
    use indoc::indoc;
    use predicates::prelude::*;
    use util::test::print;

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
                  --trace [<TRACE>]  Trace log file
              -v, --verbose...       Increase logging verbosity
              -q, --quiet...         Decrease logging verbosity
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

            Usage: variantbuilder build [OPTIONS] --eda <EDA> --placements <SOURCE> --parts <SOURCE> --part-mappings <SOURCE> --output <FILE>

            Options:
                  --eda <EDA>
                      EDA tool [possible values: diptrace, kicad]
                  --load-out <SOURCE>
                      Load-out source
                  --placements <SOURCE>
                      Placements source
              -v, --verbose...
                      Increase logging verbosity
                  --parts <SOURCE>
                      Parts source
              -q, --quiet...
                      Decrease logging verbosity
                  --part-mappings <SOURCE>
                      Part-mappings source
                  --substitutions [<SOURCE>...]
                      Substitution sources
                  --ref-des-disable-list [<REF_DES_DISABLE_LIST>...]
                      List of reference designators to disable (use for do-not-fit, no-place, test-points, fiducials, etc)
                  --assembly-rules <SOURCE>
                      Assembly rules source
                  --output <FILE>
                      Output CSV file
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
}