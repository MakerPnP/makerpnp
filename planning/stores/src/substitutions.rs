use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Error};
use eda::substitution::EdaSubstitutionRule;
use tracing::Level;
use tracing::{info, trace};
use util::source::Source;

use crate::csv::SubstitutionRecord;

pub type EdaSubstitutionsSource = Source;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_eda_substitutions(source: &EdaSubstitutionsSource) -> Result<Vec<EdaSubstitutionRule>, Error> {
    info!("Loading eda substitutions. source: {}", source);

    let path = source
        .path()
        .map_err(|error| anyhow!("Unsupported source type. cause: {:?}", error))?;

    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(path.clone())
        .with_context(|| format!("Error reading substitutions. file: {}", path.display()))?;

    let mut eda_substitutions: Vec<EdaSubstitutionRule> = vec![];

    for result in csv_reader.deserialize() {
        let record: SubstitutionRecord = result.with_context(|| "Deserializing substitution record".to_string())?;

        trace!("{:?}", record);

        let eda_substitution = record
            .build_eda_substitution()
            .with_context(|| format!("Building substitution from record. record: {:?}", record))?;

        eda_substitutions.push(eda_substitution);
    }
    Ok(eda_substitutions)
}

/// The substitution rules, made from each one's fields, see [`crate::csv::read_rows`].
pub fn build_eda_substitutions(rows: &[BTreeMap<String, String>]) -> Result<Vec<EdaSubstitutionRule>, Error> {
    rows.iter()
        .map(|row| {
            let record = SubstitutionRecord::from_fields(row.clone().into_iter().collect());
            trace!("{:?}", record);
            record
                .build_eda_substitution()
                .with_context(|| format!("Building substitution from record. record: {:?}", record))
        })
        .collect()
}

#[cfg(test)]
pub mod csv_loading_tests {
    use assert_fs::TempDir;
    use criteria::{ExactMatchCriterion, RegexMatchCriterion};
    use csv::QuoteStyle;
    use eda::substitution::{EdaSubstitutionRule, EdaSubstitutionRuleClassification, EdaSubstitutionRuleTransformItem};
    use regex::Regex;

    use crate::substitutions::load_eda_substitutions;
    use crate::substitutions::test::TestEdaSubstitutionRecord;
    use crate::substitutions::EdaSubstitutionsSource;

    #[test]
    pub fn use_exact_match_and_regex_match_criterion() -> anyhow::Result<()> {
        // given
        let temp_dir = TempDir::new()?;
        let mut test_eda_substitutions_path = temp_dir.path().to_path_buf();
        test_eda_substitutions_path.push("substitutions.csv");
        let test_eda_substitutions_source =
            EdaSubstitutionsSource::from_absolute_path(test_eda_substitutions_path.clone())?;

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_eda_substitutions_path.clone())?;

        writer.serialize(TestEdaSubstitutionRecord {
            name_pattern: Some("NAME1".to_string()),
            value_pattern: Some("VALUE1".to_string()),
            name: Some("SUBSTITUTED_NAME1".to_string()),
            value: Some("SUBSTITUTED_VALUE1".to_string()),
            comment: None,
            ..TestEdaSubstitutionRecord::diptrace_defaults()
        })?;

        writer.serialize(TestEdaSubstitutionRecord {
            name_pattern: Some("/(NAME2)/".to_string()),
            value_pattern: Some("/(VALUE2)/".to_string()),
            name: Some("SUBSTITUTED_NAME2".to_string()),
            value: Some("SUBSTITUTED_VALUE2".to_string()),
            comment: None,
            ..TestEdaSubstitutionRecord::diptrace_defaults()
        })?;

        writer.flush()?;

        // and
        let expected_result: Vec<EdaSubstitutionRule> = vec![
            EdaSubstitutionRule {
                criteria: vec![
                    Box::new(ExactMatchCriterion {
                        field_name: "name".to_string(),
                        field_pattern: "NAME1".to_string(),
                    }),
                    Box::new(ExactMatchCriterion {
                        field_name: "value".to_string(),
                        field_pattern: "VALUE1".to_string(),
                    }),
                ],
                transforms: vec![
                    EdaSubstitutionRuleTransformItem {
                        field_name: "name".to_string(),
                        field_value: "SUBSTITUTED_NAME1".to_string(),
                    },
                    EdaSubstitutionRuleTransformItem {
                        field_name: "value".to_string(),
                        field_value: "SUBSTITUTED_VALUE1".to_string(),
                    },
                ],
                classifications: vec![],
            },
            EdaSubstitutionRule {
                criteria: vec![
                    Box::new(RegexMatchCriterion {
                        field_name: "name".to_string(),
                        field_pattern: Regex::new("(NAME2)").unwrap(),
                    }),
                    Box::new(RegexMatchCriterion {
                        field_name: "value".to_string(),
                        field_pattern: Regex::new("(VALUE2)").unwrap(),
                    }),
                ],
                transforms: vec![
                    EdaSubstitutionRuleTransformItem {
                        field_name: "name".to_string(),
                        field_value: "SUBSTITUTED_NAME2".to_string(),
                    },
                    EdaSubstitutionRuleTransformItem {
                        field_name: "value".to_string(),
                        field_value: "SUBSTITUTED_VALUE2".to_string(),
                    },
                ],
                classifications: vec![],
            },
        ];

        let csv_content = std::fs::read_to_string(test_eda_substitutions_path)?;
        println!("{csv_content:}");

        // when
        let result = load_eda_substitutions(&test_eda_substitutions_source)?;

        // then
        assert_eq!(result, expected_result);

        Ok(())
    }

    #[test]
    pub fn extract_derating_warning_and_comments() -> anyhow::Result<()> {
        // given
        let temp_dir = TempDir::new()?;
        let mut test_eda_substitutions_path = temp_dir.path().to_path_buf();
        test_eda_substitutions_path.push("substitutions.csv");
        let test_eda_substitutions_source =
            EdaSubstitutionsSource::from_absolute_path(test_eda_substitutions_path.clone())?;

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_eda_substitutions_path.clone())?;

        writer.serialize(TestEdaSubstitutionRecord {
            name_pattern: Some("NAME1".to_string()),
            value_pattern: Some("VALUE1".to_string()),
            name: Some("SUBSTITUTED_NAME1".to_string()),
            value: Some("SUBSTITUTED_VALUE1".to_string()),
            comment: Some("DERATE reason1; COMMENT1 ;COMMENT2; ! ONE ;WARNING: two; WARNING THREE ".to_string()),
            ..TestEdaSubstitutionRecord::diptrace_defaults()
        })?;

        writer.flush()?;

        // and
        let expected_result: Vec<EdaSubstitutionRule> = vec![EdaSubstitutionRule {
            criteria: vec![
                Box::new(ExactMatchCriterion {
                    field_name: "name".to_string(),
                    field_pattern: "NAME1".to_string(),
                }),
                Box::new(ExactMatchCriterion {
                    field_name: "value".to_string(),
                    field_pattern: "VALUE1".to_string(),
                }),
            ],
            transforms: vec![
                EdaSubstitutionRuleTransformItem {
                    field_name: "name".to_string(),
                    field_value: "SUBSTITUTED_NAME1".to_string(),
                },
                EdaSubstitutionRuleTransformItem {
                    field_name: "value".to_string(),
                    field_value: "SUBSTITUTED_VALUE1".to_string(),
                },
            ],
            classifications: vec![
                EdaSubstitutionRuleClassification::Derate(Some("reason1".to_string())),
                EdaSubstitutionRuleClassification::Comment(Some("COMMENT1".to_string())),
                EdaSubstitutionRuleClassification::Comment(Some("COMMENT2".to_string())),
                EdaSubstitutionRuleClassification::Warning(Some("ONE".to_string())),
                EdaSubstitutionRuleClassification::Warning(Some("two".to_string())),
                EdaSubstitutionRuleClassification::Warning(Some("THREE".to_string())),
            ],
        }];

        let csv_content = std::fs::read_to_string(test_eda_substitutions_path)?;
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
#[cfg(any(test, feature = "testing"))]
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

        // other non-eda specific fields
        #[serde(skip_serializing_if = "Option::is_none")]
        pub comment: Option<String>,
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
