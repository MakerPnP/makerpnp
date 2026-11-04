use std::collections::HashMap;
#[cfg(test)]
use std::path::Path;

use anyhow::{anyhow, Context, Error};
use pnp::package::Package;
use tracing::{info, Level};
use util::source::Source;

use crate::csv::packages::build_package_from_field_map;

pub type PackagesSource = Source;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_packages(source: &PackagesSource) -> Result<Vec<Package>, Error> {
    info!("Loading packages. source: {}", source);

    let path = source
        .path()
        .map_err(|error| anyhow!("Unsupported source type. cause: {:?}", error))?;

    let file =
        std::fs::File::open(&path).with_context(|| format!("Error reading packages. file: {}", path.display()))?;
    read_packages(file).with_context(|| format!("Error reading packages. file: {}", path.display()))
}

/// The packages, from their CSV.
pub fn read_packages(reader: impl std::io::Read) -> Result<Vec<Package>, Error> {
    // Use the CSV reader directly without serialization
    let mut reader = csv::ReaderBuilder::new().from_reader(reader);

    let mut packages = Vec::new();

    // Get the headers
    let headers = reader
        .headers()?
        .iter()
        .map(|h| h.to_string())
        .collect::<Vec<_>>();

    // Various attempts were made to use the serde feature of the csv crate, but due to these issues it was abandoned:
    // 1) https://github.com/BurntSushi/rust-csv/issues/239#issuecomment-3214427317
    // 2) https://github.com/BurntSushi/rust-csv/issues/239
    //
    // Instead we have to go 'old-skool' and parse /everything/ manually. :(

    // Process each row manually
    for result in reader.records() {
        let record = result?;

        // Create a map of field name to value
        let field_map: HashMap<String, String> = headers
            .iter()
            .enumerate()
            .map(|(i, header)| (header.clone(), record.get(i).unwrap_or("").to_string()))
            .collect();

        // Build the package from the field map
        let package = build_package_from_field_map(&field_map)?;
        packages.push(package);
    }

    Ok(packages)
}

#[cfg(test)]
pub fn save_packages(packages: &[Package], output_path: &Path) -> Result<(), Error> {
    // Create headers with all possible fields
    let mut headers = stores::csv::packages::get_base_package_headers();

    // Find the maximum number of manufacturer codes across all packages
    let max_mfr_codes = packages
        .iter()
        .map(|p| p.manufacturer_codes.len())
        .max()
        .unwrap_or(0);

    // Add headers for manufacturer codes
    for i in 1..=max_mfr_codes {
        headers.push(format!("Mfr{}", i));
        headers.push(format!("MfrCode{}", i));
    }

    // Create the CSV writer
    let mut writer = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Always)
        .from_path(output_path)?;

    // Write headers
    writer.write_record(headers)?;

    // Write each package
    for package in packages {
        let mut record = Vec::new();

        // Add basic fields
        record.push(package.name.clone());
        record.push(
            package
                .lead_count
                .map_or(String::new(), |v| v.to_string()),
        );
        record.push(
            package
                .lead_pitch_mm
                .as_ref()
                .map_or(String::new(), |v| v.to_string()),
        );

        // Add dimensions
        if let Some(dimensions) = &package.dimensions_mm {
            record.push(dimensions.size_x().to_string());
            record.push(dimensions.size_y().to_string());
            record.push(dimensions.size_z().to_string());
        } else {
            record.push(String::new()); // SizeX
            record.push(String::new()); // SizeY
            record.push(String::new()); // SizeZ
        }

        record.push(
            package
                .generic_shorthand
                .clone()
                .unwrap_or_default(),
        );
        record.push(
            package
                .eia_imperial_code
                .clone()
                .unwrap_or_default(),
        );
        record.push(
            package
                .eia_metric_code
                .clone()
                .unwrap_or_default(),
        );
        record.push(
            package
                .jeita_code
                .clone()
                .unwrap_or_default(),
        );
        record.push(
            package
                .ipc7351_code
                .clone()
                .unwrap_or_default(),
        );
        record.push(
            package
                .jedec_mo_code
                .clone()
                .unwrap_or_default(),
        );
        record.push(
            package
                .jedec_package_code
                .clone()
                .unwrap_or_default(),
        );

        // Add manufacturer codes, padding with empty strings if needed
        for i in 0..max_mfr_codes {
            if i < package.manufacturer_codes.len() {
                record.push(
                    package.manufacturer_codes[i]
                        .manufacturer
                        .clone(),
                );
                record.push(
                    package.manufacturer_codes[i]
                        .code
                        .clone(),
                );
            } else {
                record.push(String::new());
                record.push(String::new());
            }
        }

        // Write the record
        writer.write_record(&record)?;
    }

    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use util::test::{build_temp_csv_file, dump_file};

    use super::*;

    #[test]
    fn test_load_packages() -> Result<(), anyhow::Error> {
        // given
        let temp_dir = tempdir()?;

        // and packages
        let (test_packages_path, _test_packages_file_name) = build_temp_csv_file(&temp_dir, "packages");
        let packages_source = PackagesSource::from_absolute_path(test_packages_path.clone())?;

        save_packages(&[Package::new("NAME1".into())], &test_packages_path)?;

        dump_file("packages", test_packages_path)?;

        // and
        let expected_packages = vec![Package::new("NAME1".into())];

        // when
        let packages = load_packages(&packages_source)?;

        assert_eq!(packages, expected_packages);

        Ok(())
    }
}
