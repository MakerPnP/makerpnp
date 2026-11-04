use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Error};
use package_mapper::package_mapping::PackageMapping;
use pnp::package::Package;
use tracing::{info, trace, Level};
use util::source::Source;

use crate::csv::PackageMappingRecord;

pub type PackageMappingsSource = Source;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_package_mappings<'packages>(
    packages: &'packages Vec<Package>,
    source: &PackageMappingsSource,
) -> Result<Vec<PackageMapping<'packages>>, Error> {
    info!("Loading package mappings. source: {}", source);

    let path = source
        .path()
        .map_err(|error| anyhow!("Unsupported source type. cause: {:?}", error))?;

    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(path.clone())
        .with_context(|| format!("Error reading package mappings. file: {}", path.display()))?;

    let mut package_mappings: Vec<PackageMapping> = vec![];

    for result in csv_reader.deserialize() {
        let record: PackageMappingRecord =
            result.with_context(|| "Deserializing package mapping record".to_string())?;

        trace!("{:?}", record);

        let part_mapping = record
            .build_package_mapping(packages)
            .with_context(|| format!("Building package mapping from record. record: {:?}", record))?;

        package_mappings.push(part_mapping);
    }
    Ok(package_mappings)
}

/// Each package mapping's fields, by name, from their CSV, see [`build_package_mappings`].
pub fn read_package_mapping_rows(reader: impl std::io::Read) -> Result<Vec<BTreeMap<String, String>>, Error> {
    let mut csv_reader = csv::ReaderBuilder::new().from_reader(reader);

    let mut rows = vec![];
    for result in csv_reader.deserialize() {
        let row: BTreeMap<String, String> =
            result.with_context(|| "Deserializing package mapping record".to_string())?;
        rows.push(row);
    }
    Ok(rows)
}

/// The package mappings, made from each one's fields and the packages they map to.
pub fn build_package_mappings<'packages>(
    packages: &'packages [Package],
    rows: &[BTreeMap<String, String>],
) -> Result<Vec<PackageMapping<'packages>>, Error> {
    rows.iter()
        .map(|row| {
            let record = PackageMappingRecord::from_fields(row.clone().into_iter().collect());
            trace!("{:?}", record);
            record
                .build_package_mapping(packages)
                .with_context(|| format!("Building package mapping from record. record: {:?}", record))
        })
        .collect()
}

#[cfg(test)]
pub mod csv_loading_tests {
    use assert_fs::TempDir;
    use criteria::{ExactMatchCriterion, GenericCriteria, RegexMatchCriterion};
    use csv::QuoteStyle;
    use regex::Regex;
    use test::TestPackageMappingRecord;

    use super::*;
    use crate::packages::PackagesSource;

    #[test]
    pub fn use_exact_match_and_regex_match_criterion() -> anyhow::Result<()> {
        // given
        let packages: Vec<Package> = vec![Package::new("NAME1".into())];

        // and
        let temp_dir = TempDir::new()?;
        let mut test_package_mappings_path = temp_dir.path().to_path_buf();
        test_package_mappings_path.push("package-mappings.csv");
        let test_package_mappings_source = PackagesSource::from_absolute_path(test_package_mappings_path.clone())?;

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_path(test_package_mappings_path.clone())?;

        writer.serialize(TestPackageMappingRecord {
            manufacturer: "424242".to_string(),
            mpn: "696969".to_string(),
            // maps to
            name: "NAME1".to_string(),
            ..TestPackageMappingRecord::default()
        })?;

        writer.serialize(TestPackageMappingRecord {
            manufacturer: "424242".to_string(),
            mpn: "/.*/".to_string(),
            // maps to
            name: "NAME1".to_string(),
            ..TestPackageMappingRecord::default()
        })?;

        writer.flush()?;

        // and
        let expected_result: Vec<PackageMapping> = vec![
            PackageMapping {
                package: &packages[1 - 1],
                criteria: vec![Box::new(GenericCriteria {
                    criteria: vec![
                        Box::new(ExactMatchCriterion {
                            field_name: "manufacturer".to_string(),
                            field_pattern: "424242".to_string(),
                        }),
                        Box::new(ExactMatchCriterion {
                            field_name: "mpn".to_string(),
                            field_pattern: "696969".to_string(),
                        }),
                    ],
                })],
            },
            PackageMapping {
                package: &packages[1 - 1],
                criteria: vec![Box::new(GenericCriteria {
                    criteria: vec![
                        Box::new(ExactMatchCriterion {
                            field_name: "manufacturer".to_string(),
                            field_pattern: "424242".to_string(),
                        }),
                        Box::new(RegexMatchCriterion {
                            field_name: "mpn".to_string(),
                            field_pattern: Regex::new(".*").unwrap(),
                        }),
                    ],
                })],
            },
        ];

        let csv_content = std::fs::read_to_string(test_package_mappings_path)?;
        println!("{csv_content:}");

        // when
        let result = load_package_mappings(&packages, &test_package_mappings_source)?;

        // then
        assert_eq!(result, expected_result);

        Ok(())
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod test {
    #[derive(Debug, Default, serde::Serialize)]
    #[serde(rename_all(serialize = "PascalCase"))]
    pub struct TestPackageMappingRecord {
        //
        // From
        //
        pub manufacturer: String,
        pub mpn: String,

        //
        // To
        //
        pub name: String,
    }
}
