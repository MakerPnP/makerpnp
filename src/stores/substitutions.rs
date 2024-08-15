use tracing::Level;
use std::path::PathBuf;
use anyhow::{Context, Error};
use tracing::trace;
use crate::eda::substitution::EdaSubstitutionRule;
use crate::stores::csv::SubstitutionRecord;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_eda_substitutions(substitutions_source: &String) -> Result<Vec<EdaSubstitutionRule>, Error> {
    let substitutions_path_buf = PathBuf::from(substitutions_source);
    let substitutions_path = substitutions_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(substitutions_path)
        .with_context(|| format!("Error reading substitutions. file: {}", substitutions_path.to_str().unwrap()))?;


    let mut eda_substitutions: Vec<EdaSubstitutionRule> = vec![];

    for result in csv_reader.deserialize() {
        let record: SubstitutionRecord = result
            .with_context(|| "Deserializing substitution record".to_string())?;

        trace!("{:?}", record);

        let eda_substitution = record.build_eda_substitution()
            .with_context(|| format!("Building substitution from record. record: {:?}", record))?;

        eda_substitutions.push(eda_substitution);

    }
    Ok(eda_substitutions)
}

#[cfg(test)]
pub mod csv_loading_tests {
    use assert_fs::TempDir;
    use csv::QuoteStyle;
    use regex::Regex;
    use crate::eda::criteria::{ExactMatchCriterion, RegexMatchCriterion};
    use crate::eda::substitution::{EdaSubstitutionRule, EdaSubstitutionRuleTransformItem};
    use crate::stores::substitutions::load_eda_substitutions;
    use crate::stores::substitutions::test::TestEdaSubstitutionRecord;

    #[test]
    pub fn use_exact_match_and_regex_match_criterion() -> anyhow::Result<()>{
        // given
        let temp_dir = TempDir::new()?;
        let mut test_eda_substitutions_path = temp_dir.path().to_path_buf();
        test_eda_substitutions_path.push("substitutions.csv");
        let test_eda_substitutions_source = test_eda_substitutions_path.to_str().unwrap().to_string();

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_eda_substitutions_path)?;

        writer.serialize(TestEdaSubstitutionRecord {
            name_pattern: Some("NAME1".to_string()),
            value_pattern: Some("VALUE1".to_string()),
            name: Some("SUBSTITUTED_NAME1".to_string()),
            value: Some("SUBSTITUTED_VALUE1".to_string()),
            ..TestEdaSubstitutionRecord::diptrace_defaults()
        })?;

        writer.serialize(TestEdaSubstitutionRecord {
            name_pattern: Some("/(NAME2)/".to_string()),
            value_pattern: Some("/(VALUE2)/".to_string()),
            name: Some("SUBSTITUTED_NAME2".to_string()),
            value: Some("SUBSTITUTED_VALUE2".to_string()),
            ..TestEdaSubstitutionRecord::diptrace_defaults()
        })?;

        writer.flush()?;

        // and
        let expected_result: Vec<EdaSubstitutionRule> = vec![
            EdaSubstitutionRule {
                criteria: vec![
                    Box::new(ExactMatchCriterion { field_name: "name".to_string(), field_pattern: "NAME1".to_string() }),
                    Box::new(ExactMatchCriterion { field_name: "value".to_string(), field_pattern: "VALUE1".to_string() }),
                ],
                transforms: vec![
                    EdaSubstitutionRuleTransformItem { field_name: "name".to_string(), field_value: "SUBSTITUTED_NAME1".to_string() }, 
                    EdaSubstitutionRuleTransformItem { field_name: "value".to_string(), field_value: "SUBSTITUTED_VALUE1".to_string() }
                ],
            },
            EdaSubstitutionRule {
                criteria: vec![
                    Box::new(RegexMatchCriterion { field_name: "name".to_string(), field_pattern: Regex::new("(NAME2)").unwrap() }),
                    Box::new(RegexMatchCriterion { field_name: "value".to_string(), field_pattern: Regex::new("(VALUE2)").unwrap() }),
                ],
                transforms: vec![
                    EdaSubstitutionRuleTransformItem { field_name: "name".to_string(), field_value: "SUBSTITUTED_NAME2".to_string() },
                    EdaSubstitutionRuleTransformItem { field_name: "value".to_string(), field_value: "SUBSTITUTED_VALUE2".to_string() }
                ],
            }
        ];

        let csv_content = std::fs::read_to_string(test_eda_substitutions_source.clone())?;
        println!("{csv_content:}");

        // when
        let result = load_eda_substitutions(&test_eda_substitutions_source)?;

        // then
        assert_eq!(result, expected_result);

        Ok(())
    }
}


// FUTURE Ideally we want to include this module ONLY for integration tests or for unit tests
//        but when compiling for integration tests, `test` is NOT defined so we cannot use
//        just `#[cfg(test)]`
#[cfg(any(test, feature="cli"))]
pub mod test {
    #[derive(Debug, Default, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    pub struct TestEdaSubstitutionRecord {
        pub eda: String,

        // DipTrace specific
        #[serde(skip_serializing_if = "Option::is_none")]
        pub name_pattern: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub value_pattern: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,

        // KiCad specific
        #[serde(skip_serializing_if = "Option::is_none")]
        pub package_pattern: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub val_pattern: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub package: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub val: Option<String>,
    }

    impl TestEdaSubstitutionRecord {
        pub fn diptrace_defaults() -> TestEdaSubstitutionRecord {
            TestEdaSubstitutionRecord {
                eda: "DipTrace".to_string(),
                ..Default::default()
            }
        }

        pub fn kicad_defaults() -> TestEdaSubstitutionRecord {
            TestEdaSubstitutionRecord {
                eda: "KiCad".to_string(),
                ..Default::default()
            }
        }
    }
}