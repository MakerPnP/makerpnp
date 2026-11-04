use std::collections::HashMap;

use assembly::rules::AssemblyRule;
use criteria::{ExactMatchCriterion, FieldCriterion, GenericCriteria, RegexMatchCriterion};
use eda::substitution::{EdaSubstitutionRule, EdaSubstitutionRuleClassification, EdaSubstitutionRuleTransformItem};
use eda::EdaTool;
use heck::ToUpperCamelCase;
use package_mapper::criteria::PartMappingCriteria;
use package_mapper::package_mapping::PackageMapping;
use part_mapper::criteria::PlacementMappingCriteria;
use part_mapper::part_mapping::PartMapping;
use pnp::load_out::LoadOutItem;
use pnp::package::Package;
use pnp::part::Part;
use pnp::reference::Reference;
use regex::{Error, Regex};
use thiserror::Error;

pub mod packages;

/// Each record's fields, by header, from a CSV whose records are made by a core, such as part mappings (`STORE-11`).
pub fn read_rows(reader: impl std::io::Read) -> anyhow::Result<Vec<std::collections::BTreeMap<String, String>>> {
    let mut csv_reader = csv::ReaderBuilder::new().from_reader(reader);

    let mut rows = vec![];
    for result in csv_reader.deserialize() {
        let row: std::collections::BTreeMap<String, String> = result.map_err(anyhow::Error::from)?;
        rows.push(row);
    }
    Ok(rows)
}

// FUTURE Investigate whether the `build` methods should be taking `self` instead of `&self` to avoid additional allocations
//        Most of the time records are parsed, then domain objects are built based on the records then the records
//        is discarded.

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct PackageMappingRecord(HashMap<String, String>);

impl PackageMappingRecord {
    /// A record from its fields, by name, as read from a CSV row.
    pub fn from_fields(fields: HashMap<String, String>) -> Self {
        Self(fields)
    }

    pub fn build_package_mapping<'package>(
        &self,
        packages: &'package [Package],
    ) -> Result<PackageMapping<'package>, PackageMappingRecordError> {
        let fields = &self.0;

        let name = fields
            .get("Name")
            .ok_or(PackageMappingRecordError::MissingField {
                field: "Name".to_string(),
            })?;

        let matched_package_ref = packages
            .iter()
            .find(|&package| package.name.eq(name));

        let package_ref = match matched_package_ref {
            Some(package) => Ok(package),
            _ => Err(PackageMappingRecordError::NoMatchingPackage {
                criteria: name.clone(),
            }),
        }?;

        let fields_names = ["manufacturer", "mpn"];

        let matched_fields = build_matched_fields(fields, &fields_names);

        let mut mapping_criteria: Vec<Box<dyn PartMappingCriteria>> = vec![];

        let criteria_fields: Vec<Box<dyn FieldCriterion>> =
            matched_fields
                .iter()
                .try_fold(vec![], |mut acc, (&ref key, &ref value)| {
                    let value_kind =
                        build_value_kind(&value).map_err(|error| PackageMappingRecordError::InvalidRegex {
                            error,
                        })?;

                    let boxed_criterion: Box<dyn FieldCriterion> = match value_kind {
                        ValueKind::Regex(regex) => Box::new(RegexMatchCriterion::new(key.to_lowercase(), regex)),
                        ValueKind::ExactMatch(value) => Box::new(ExactMatchCriterion::new(key.to_lowercase(), value)),
                    };
                    acc.push(boxed_criterion);
                    Ok(acc)
                })?;
        let criteria = GenericCriteria {
            criteria: criteria_fields,
        };

        mapping_criteria.push(Box::new(criteria));

        let package_mapping = PackageMapping::new(package_ref, mapping_criteria);

        Ok(package_mapping)
    }
}

#[derive(Error, Debug)]
pub enum PackageMappingRecordError {
    #[error("Unable to build criteria")]
    UnableToBuildCriteria,

    #[error("No matching package, criteria: {criteria:?}")]
    NoMatchingPackage { criteria: String },

    #[error("Missing field. field: {field:?}")]
    MissingField { field: String },

    #[error("Invalid regular expression. reason: {error:?}")]
    InvalidRegex { error: regex::Error },
}

// FUTURE Investigate if it's possible to specify required fields like 'Eda', 'Manufacturer', 'Mpn' and then a hashmap
//       for the remaining, eda-specific, fields.  Maybe using `#[serde(flatten)] eda_fields: HashMap<String, String>`.
//       likely it's probably not, due to this bugs that would need resolving first:
//       1) https://github.com/BurntSushi/rust-csv/issues/239
//       2) https://github.com/BurntSushi/rust-csv/issues/344#issuecomment-2286126491
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct PartMappingRecord(HashMap<String, String>);

#[derive(Error, Debug)]
pub enum PartMappingRecordError {
    #[error("Unable to build criteria")]
    UnableToBuildCriteria,

    #[error("No matching part, criteria: {criteria:?}")]
    NoMatchingPart { criteria: Part },

    #[error("Unknown EDA. value: {eda:?}")]
    UnknownEda { eda: String },

    #[error("Missing field. field: {field:?}")]
    MissingField { field: String },

    #[error("Invalid regular expression. reason: {error:?}")]
    InvalidRegex { error: regex::Error },
}

impl PartMappingRecord {
    /// A record from its fields, by name, as read from a CSV row.
    pub fn from_fields(fields: HashMap<String, String>) -> Self {
        Self(fields)
    }

    pub fn build_part_mapping<'part>(
        &self,
        parts: &'part [Part],
    ) -> Result<PartMapping<'part>, PartMappingRecordError> {
        // NOTE: Initially the PartMappingRecord had more properties and was using serde flatten on the fields but there was a bug;
        //       so we have to do some deserialization manually instead.
        //       See https://github.com/BurntSushi/rust-csv/issues/344#issuecomment-2286126491

        let fields = &self.0;

        let eda = fields
            .get("Eda")
            .ok_or(PartMappingRecordError::MissingField {
                field: "Eda".to_string(),
            })?;
        let manufacturer = fields
            .get("Manufacturer")
            .ok_or(PartMappingRecordError::MissingField {
                field: "Manufacturer".to_string(),
            })?;
        let mpn = fields
            .get("Mpn")
            .ok_or(PartMappingRecordError::MissingField {
                field: "Mpn".to_string(),
            })?;

        let eda = csv_eda_tool_value_to_eda_tool(eda).ok_or(PartMappingRecordError::UnknownEda {
            eda: eda.clone(),
        })?;

        let part_criteria: Part = Part {
            manufacturer: manufacturer.clone(),
            mpn: mpn.clone(),
        };

        let matched_part_ref = parts
            .iter()
            .find(|&part| part.eq(&part_criteria));

        let part_ref = match matched_part_ref {
            Some(part) => Ok(part),
            _ => Err(PartMappingRecordError::NoMatchingPart {
                criteria: part_criteria,
            }),
        }?;

        let fields_names = eda_fields_names(&eda);

        let matched_fields = build_matched_fields(fields, fields_names);

        let mut mapping_criteria: Vec<Box<dyn PlacementMappingCriteria>> = vec![];

        let criteria_fields: Vec<Box<dyn FieldCriterion>> =
            matched_fields
                .iter()
                .try_fold(vec![], |mut acc, (&ref key, &ref value)| {
                    let value_kind =
                        build_value_kind(&value).map_err(|error| PartMappingRecordError::InvalidRegex {
                            error,
                        })?;

                    let boxed_criterion: Box<dyn FieldCriterion> = match value_kind {
                        ValueKind::Regex(regex) => Box::new(RegexMatchCriterion::new(key.to_lowercase(), regex)),
                        ValueKind::ExactMatch(value) => Box::new(ExactMatchCriterion::new(key.to_lowercase(), value)),
                    };
                    acc.push(boxed_criterion);
                    Ok(acc)
                })?;
        let criteria = GenericCriteria {
            criteria: criteria_fields,
        };

        mapping_criteria.push(Box::new(criteria));

        let part_mapping = PartMapping::new(part_ref, mapping_criteria);

        Ok(part_mapping)
    }
}

pub enum ValueKind {
    Regex(Regex),
    ExactMatch(String),
}

pub fn build_value_kind(value: &str) -> Result<ValueKind, Error> {
    if value.starts_with('/') && value.ends_with('/') {
        let (_prefix, remainder) = value.split_at(1);
        let mut value = remainder.to_string();
        value.pop();

        let regex = Regex::new(&value)?;

        Ok(ValueKind::Regex(regex))
    } else {
        Ok(ValueKind::ExactMatch(value.to_string()))
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoadOutItemRecord {
    pub reference: Option<Reference>,
    pub manufacturer: String,
    pub mpn: String,
}

impl LoadOutItemRecord {
    pub fn build_load_out_item(&self) -> Result<LoadOutItem, anyhow::Error> {
        Ok(LoadOutItem {
            reference: self.reference.clone(),
            manufacturer: self.manufacturer.clone(),
            mpn: self.mpn.clone(),
        })
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct SubstitutionRecord(HashMap<String, String>);

#[derive(Error, Debug)]
pub enum SubstitutionRecordError {
    #[error("Field mismatch, expected: {0:?}")]
    FieldMismatch(Vec<String>),

    #[error("Unknown EDA. value: {eda:?}")]
    UnknownEda { eda: String },

    #[error("Missing field. field: {field:?}")]
    MissingField { field: String },

    #[error("Invalid regular expression. reason: {error:?}")]
    InvalidRegex { error: regex::Error },
}

impl SubstitutionRecord {
    /// A record from its fields, by name, as read from a CSV row.
    pub fn from_fields(fields: HashMap<String, String>) -> Self {
        Self(fields)
    }

    pub fn build_eda_substitution(&self) -> anyhow::Result<EdaSubstitutionRule, SubstitutionRecordError> {
        // NOTE: Initially the SubstitutionRecord had more properties and was using serde flatten on the fields but there was a bug;
        //       so we have to do some deserialization manually instead.
        //       See https://github.com/BurntSushi/rust-csv/issues/344#issuecomment-2286126491

        let fields = &self.0;

        let eda = fields
            .get("Eda")
            .ok_or(SubstitutionRecordError::MissingField {
                field: "Eda".to_string(),
            })?;

        let eda = csv_eda_tool_value_to_eda_tool(eda).ok_or(SubstitutionRecordError::UnknownEda {
            eda: eda.clone(),
        })?;

        let fields_names = eda_fields_names(&eda);

        let mut criteria: Vec<Box<dyn FieldCriterion>> = vec![];
        let mut transforms: Vec<EdaSubstitutionRuleTransformItem> = vec![];

        for &field_name in fields_names.iter() {
            // Note: heck's UpperCamelCase appears to be the same as serde's PascalCase
            //       however we can't use serde's case transforms as they are internal to serde.
            //       see serde_derive::internals::case::RenameRule

            let name_field = field_name.to_upper_camel_case();
            let pattern_field = format!("{}_pattern", field_name).to_upper_camel_case();

            match (fields.get(&name_field), fields.get(&pattern_field)) {
                (Some(field_name_value), Some(pattern_value)) => {
                    let value_kind =
                        build_value_kind(&pattern_value).map_err(|error| SubstitutionRecordError::InvalidRegex {
                            error,
                        })?;

                    let boxed_criterion: Box<dyn FieldCriterion> = match value_kind {
                        ValueKind::Regex(regex) => Box::new(RegexMatchCriterion {
                            field_name: field_name.to_string(),
                            field_pattern: regex,
                        }),
                        ValueKind::ExactMatch(value) => Box::new(ExactMatchCriterion {
                            field_name: field_name.to_string(),
                            field_pattern: value,
                        }),
                    };
                    criteria.push(boxed_criterion);
                    transforms.push(EdaSubstitutionRuleTransformItem {
                        field_name: field_name.to_string(),
                        field_value: field_name_value.to_string(),
                    });
                }
                _ => return Err(SubstitutionRecordError::FieldMismatch(vec![name_field, pattern_field])),
            }
        }

        let comment = fields.get("Comment").cloned();

        let mut classifications = vec![];

        if let Some(comment) = comment {
            let parts = comment.split(";");
            for part in parts {
                let part = part.trim();
                /// Case insensitively strip any prefixes and return the remainder, or None if none of the prefixes were found
                ///
                /// Uses the first prefix that matches. Caller should take care to order prefixes when using
                /// prefixes that are similar.  e.g. use `["TESTING", "TEST"]` (longest first), not `["TEST", "TESTING"]`.
                fn strip_any_prefix_case_insensitive<'a>(s: &'a str, prefixes: &[&str]) -> Option<&'a str> {
                    prefixes.iter().find_map(|p| {
                        let head = s.get(..p.len())?;
                        if head.eq_ignore_ascii_case(p) {
                            Some(&s[p.len()..])
                        } else {
                            None
                        }
                    })
                }

                if let Some(remaining) = strip_any_prefix_case_insensitive(&part, &["DERATE", "DE-RATE"]) {
                    classifications.push(EdaSubstitutionRuleClassification::Derate(Some(
                        remaining.trim().to_string(),
                    )));
                    continue;
                }

                if let Some(remaining) = strip_any_prefix_case_insensitive(&part, &["WARNING:", "WARNING", "!"]) {
                    classifications.push(EdaSubstitutionRuleClassification::Warning(Some(
                        remaining.trim().to_string(),
                    )));
                    continue;
                }

                classifications.push(EdaSubstitutionRuleClassification::Comment(Some(
                    part.trim().to_string(),
                )));
            }
        };

        Ok(EdaSubstitutionRule {
            criteria,
            transforms,
            classifications,
        })
    }
}

fn eda_fields_names(eda: &EdaTool) -> &'static [&'static str] {
    match eda {
        EdaTool::DipTrace => &["name", "value"],
        EdaTool::KiCad => &["package", "val"],
        EdaTool::EasyEda => &["device", "value"],
    }
}

fn csv_eda_tool_value_to_eda_tool(eda: &String) -> Option<EdaTool> {
    if eda.to_upper_camel_case().eq("DipTrace") {
        Some(EdaTool::DipTrace)
    } else if eda.to_upper_camel_case().eq("KiCad") {
        Some(EdaTool::KiCad)
    } else if eda.to_upper_camel_case().eq("EasyEda") {
        Some(EdaTool::EasyEda)
    } else {
        None
    }
}

#[derive(Error, Debug)]
pub enum CSVSubstitutionRecordError {
    #[error("Unknown EDA: '{eda:}'")]
    UnknownEDA { eda: String },
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct AssemblyRuleRecord {
    ref_des: String,
    manufacturer: String,
    mpn: String,
}

impl AssemblyRuleRecord {
    /// A record from its fields, by name, as read from a CSV row.
    pub fn from_fields(fields: &HashMap<String, String>) -> Result<Self, anyhow::Error> {
        let field = |name: &str| {
            fields
                .get(name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Missing field. field: {:?}", name))
        };
        Ok(Self {
            ref_des: field("RefDes")?,
            manufacturer: field("Manufacturer")?,
            mpn: field("Mpn")?,
        })
    }

    pub fn build_assembly_rule(&self) -> Result<AssemblyRule, anyhow::Error> {
        Ok(AssemblyRule {
            ref_des: self.ref_des.clone(),
            manufacturer: self.manufacturer.clone(),
            mpn: self.mpn.clone(),
        })
    }
}

fn build_matched_fields<'a>(
    fields: &'a HashMap<String, String>,
    fields_names: &[&str],
) -> Vec<(&'a String, &'a String)> {
    let mut matched_fields: Vec<(&String, &String)> = fields
        .iter()
        .filter_map(
            |(key, value)| match fields_names.contains(&key.to_lowercase().as_str()) {
                true => Some((key, value)),
                false => None,
            },
        )
        .collect();

    matched_fields.sort();
    matched_fields
}
