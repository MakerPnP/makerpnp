use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Error};
use part_mapper::part_mapping::PartMapping;
use pnp::part::Part;
use tracing::Level;
use tracing::{info, trace};
use util::source::Source;

use crate::csv::PartMappingRecord;

pub type PartMappingsSource = Source;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_part_mappings<'part>(
    parts: &'part Vec<Part>,
    source: &PartMappingsSource,
) -> Result<Vec<PartMapping<'part>>, Error> {
    info!("Loading part mappings. source: {}", source);

    let path = source
        .path()
        .map_err(|error| anyhow!("Unsupported source type. cause: {:?}", error))?;

    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(path.clone())
        .with_context(|| format!("Error reading part mappings. file: {}", path.display()))?;

    let mut part_mappings: Vec<PartMapping> = vec![];

    for result in csv_reader.deserialize() {
        let record: PartMappingRecord = result.with_context(|| "Deserializing part mapping record".to_string())?;

        trace!("{:?}", record);

        let part_mapping = record
            .build_part_mapping(parts)
            .with_context(|| format!("Building part mapping from record. record: {:?}", record))?;

        part_mappings.push(part_mapping);
    }
    Ok(part_mappings)
}

/// The part mappings, made from each one's fields, see [`crate::csv::read_rows`], and the parts they map to.
pub fn build_part_mappings<'part>(
    parts: &'part [Part],
    rows: &[BTreeMap<String, String>],
) -> Result<Vec<PartMapping<'part>>, Error> {
    rows.iter()
        .map(|row| {
            let record = PartMappingRecord::from_fields(row.clone().into_iter().collect());
            trace!("{:?}", record);
            record
                .build_part_mapping(parts)
                .with_context(|| format!("Building part mapping from record. record: {:?}", record))
        })
        .collect()
}

#[cfg(test)]
pub mod csv_loading_tests {
    use assert_fs::TempDir;
    use criteria::{ExactMatchCriterion, GenericCriteria, RegexMatchCriterion};
    use csv::QuoteStyle;
    use part_mapper::part_mapping::PartMapping;
    use pnp::part::Part;
    use regex::Regex;

    use crate::packages::PackagesSource;
    use crate::part_mappings::load_part_mappings;
    use crate::part_mappings::test::TestPartMappingRecord;

    /// Regression test for workaround to the serde flatten issue.
    /// See https://github.com/BurntSushi/rust-csv/issues/344#issuecomment-2286126491
    #[test]
    pub fn ensure_fields_containing_integers_can_be_loaded() -> anyhow::Result<()> {
        // given
        let parts: Vec<Part> = vec![Part {
            manufacturer: "424242".to_string(),
            mpn: "696969".to_string(),
        }];

        // and
        let temp_dir = TempDir::new()?;
        let mut test_part_mappings_path = temp_dir.path().to_path_buf();
        test_part_mappings_path.push("part-mappings.csv");
        let test_part_mappings_source = PackagesSource::from_absolute_path(test_part_mappings_path.clone())?;

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_part_mappings_path.clone())?;

        writer.serialize(TestPartMappingRecord {
            name: Some("12345".to_string()),
            value: Some("54321".to_string()),
            // maps to
            manufacturer: "424242".to_string(),
            mpn: "696969".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;

        writer.flush()?;

        let csv_content = std::fs::read_to_string(&test_part_mappings_path)?;
        println!("{csv_content:}");

        // when
        let result = load_part_mappings(&parts, &test_part_mappings_source);

        // then
        assert!(result.is_ok());

        Ok(())
    }

    #[test]
    pub fn use_exact_match_and_regex_match_criterion() -> anyhow::Result<()> {
        // given
        let parts: Vec<Part> = vec![Part {
            manufacturer: "424242".to_string(),
            mpn: "696969".to_string(),
        }];

        // and
        let temp_dir = TempDir::new()?;
        let mut test_part_mappings_path = temp_dir.path().to_path_buf();
        test_part_mappings_path.push("part-mappings.csv");
        let test_part_mappings_source = PackagesSource::from_absolute_path(test_part_mappings_path.clone())?;

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_part_mappings_path.clone())?;

        writer.serialize(TestPartMappingRecord {
            name: Some("12345".to_string()),
            value: Some("54321".to_string()),
            // maps to
            manufacturer: "424242".to_string(),
            mpn: "696969".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;

        writer.serialize(TestPartMappingRecord {
            name: Some("12345".to_string()),
            value: Some("/.*/".to_string()),
            // maps to
            manufacturer: "424242".to_string(),
            mpn: "696969".to_string(),
            ..TestPartMappingRecord::diptrace_defaults()
        })?;

        writer.flush()?;

        // and
        let expected_result: Vec<PartMapping> = vec![
            PartMapping {
                part: parts.get(0).unwrap(),
                criteria: vec![Box::new(GenericCriteria {
                    criteria: vec![
                        Box::new(ExactMatchCriterion {
                            field_name: "name".to_string(),
                            field_pattern: "12345".to_string(),
                        }),
                        Box::new(ExactMatchCriterion {
                            field_name: "value".to_string(),
                            field_pattern: "54321".to_string(),
                        }),
                    ],
                })],
            },
            PartMapping {
                part: parts.get(0).unwrap(),
                criteria: vec![Box::new(GenericCriteria {
                    criteria: vec![
                        Box::new(ExactMatchCriterion {
                            field_name: "name".to_string(),
                            field_pattern: "12345".to_string(),
                        }),
                        Box::new(RegexMatchCriterion {
                            field_name: "value".to_string(),
                            field_pattern: Regex::new(".*").unwrap(),
                        }),
                    ],
                })],
            },
        ];

        let csv_content = std::fs::read_to_string(test_part_mappings_path)?;
        println!("{csv_content:}");

        // when
        let result = load_part_mappings(&parts, &test_part_mappings_source)?;

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
    pub struct TestPartMappingRecord {
        //
        // From
        //
        pub eda: String,

        // DipTrace specific
        #[serde(skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,

        // KiCad specific
        #[serde(skip_serializing_if = "Option::is_none")]
        pub package: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub val: Option<String>,

        // EasyEda specific
        #[serde(skip_serializing_if = "Option::is_none")]
        pub device: Option<String>,
        // Shares 'value' with DipTrace

        //
        // To
        //
        pub manufacturer: String,
        pub mpn: String,
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

        pub fn easyeda_defaults() -> TestPartMappingRecord {
            TestPartMappingRecord {
                eda: "EasyEda".to_string(),
                ..Default::default()
            }
        }
    }
}
