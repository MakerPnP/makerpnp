use std::collections::HashMap;
use thiserror::Error;
use heck::ToUpperCamelCase;
use crate::assembly::rules::AssemblyRule;
use crate::eda::criteria::{GenericCriteriaItem, GenericExactMatchCriteria};
use crate::eda::EdaTool;
use crate::eda::substitution::{EdaSubstitutionRule, EdaSubstitutionRuleTransformItem, EdaSubstitutionRuleCriteriaItem};
use crate::part_mapper::criteria::PlacementMappingCriteria;
use crate::part_mapper::part_mapping::PartMapping;
use crate::pnp::part::Part;
use crate::pnp::load_out::LoadOutItem;

#[derive(Debug, serde::Deserialize)]
enum CSVEdaToolValue {
    DipTrace,
    KiCad,
}

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
}

impl PartMappingRecord {
    pub fn build_part_mapping<'part>(&self, parts: &'part [Part]) -> Result<PartMapping<'part>, PartMappingRecordError> {

        // NOTE: Initially the PartMappingRecord had more properties and was using serde flatten on the fields but there was a bug;
        //       so we have to do some deserialization manually instead.
        //       See https://github.com/BurntSushi/rust-csv/issues/344#issuecomment-2286126491

        let eda = self.0.get("Eda")
            .ok_or(PartMappingRecordError::MissingField{ field: "Eda".to_string() })?;
        let manufacturer = self.0.get("Manufacturer")
            .ok_or(PartMappingRecordError::MissingField{ field: "Manufacturer".to_string() })?;
        let mpn = self.0.get("Mpn")
            .ok_or(PartMappingRecordError::MissingField{ field: "Mpn".to_string() })?;

        let eda = if eda.to_upper_camel_case().eq("DipTrace") {
            Ok(EdaTool::DipTrace)
        } else if eda.to_upper_camel_case().eq("KiCad") {
            Ok(EdaTool::KiCad)
        } else {
            Err(PartMappingRecordError::UnknownEda { eda: eda.clone() })
        }?;

        let mut fields = self.0.clone();
        fields.remove("EdaTool");
        fields.remove("Manufacturer");
        fields.remove("Mpn");


        let part_criteria: Part = Part { manufacturer: manufacturer.clone(), mpn: mpn.clone() };

        let matched_part_ref = parts.iter().find(|&part| {
            part.eq(&part_criteria)
        });

        let part_ref = match matched_part_ref {
            Some(part) => Ok(part),
            _ => Err(PartMappingRecordError::NoMatchingPart { criteria: part_criteria })
        }?;

        let mut mapping_criteria: Vec<Box<dyn PlacementMappingCriteria>> = vec![];

        let criteria_fields = fields.iter().filter(|(key, _value)|{
            match eda {
                EdaTool::DipTrace => ["name", "value"].contains(&key.to_lowercase().as_str()),
                EdaTool::KiCad => ["package", "val"].contains(&key.to_lowercase().as_str()),
            }
        }).map(|(key,value)| {
            GenericCriteriaItem::new(key.to_lowercase(), value.clone())
        }).collect();
        let criteria = GenericExactMatchCriteria { criteria: criteria_fields };

        mapping_criteria.push(Box::new(criteria));

        let part_mapping = PartMapping::new(part_ref, mapping_criteria);

        Ok(part_mapping)
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct PartRecord {
    manufacturer: String,
    mpn: String,
}

impl PartRecord {
    pub fn build_part(&self) -> Result<Part, anyhow::Error> {
        Ok(Part {
            manufacturer: self.manufacturer.clone(),
            mpn: self.mpn.clone(),
        })
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LoadOutItemRecord {
    pub reference: String,
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
pub struct SubstitutionRecord {
    eda: CSVEdaToolValue,

    #[serde(flatten)]
    fields: HashMap<String, String>,
}

#[derive(Error, Debug)]
pub enum SubstitutionRecordError {
    #[error("Field mismatch, expected: {0:?}")]
    FieldMismatch(Vec<String>)
}

impl SubstitutionRecord {
    pub fn build_eda_substitution(&self) -> Result<EdaSubstitutionRule, SubstitutionRecordError> {

        let fields_names = match self.eda {
            CSVEdaToolValue::DipTrace => ["name","value"],
            CSVEdaToolValue::KiCad => ["package","val"],
        };

        let mut criteria: Vec<EdaSubstitutionRuleCriteriaItem> = vec![];
        let mut transforms: Vec<EdaSubstitutionRuleTransformItem> = vec![];

        for &field_name in fields_names.iter() {
            // Note: heck's UpperCamelCase appears to be the same as serde's PascalCase
            //       however we can't use serde's case transforms as they are internal to serde.
            //       see serde_derive::internals::case::RenameRule

            let name_field = field_name.to_upper_camel_case();
            let pattern_field = format!("{}_pattern", field_name).to_upper_camel_case();

            match (self.fields.get(&name_field), self.fields.get(&pattern_field)) {
                (Some(field_name_value), Some(pattern_value)) => {
                    criteria.push(EdaSubstitutionRuleCriteriaItem { field_name: field_name.to_string(), field_pattern: pattern_value.to_string() } );
                    transforms.push(EdaSubstitutionRuleTransformItem { field_name: field_name.to_string(), field_value: field_name_value.to_string() } );
                },
                _ => return Err(SubstitutionRecordError::FieldMismatch(vec![name_field, pattern_field])),
            }
        }

        Ok(EdaSubstitutionRule { criteria, transforms })
    }
}

#[derive(Error, Debug)]
pub enum CSVSubstitutionRecordError {
    #[error("Unknown EDA: '{eda:}'")]
    UnknownEDA { eda: String }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct AssemblyRuleRecord {
    ref_des: String,
    manufacturer: String,
    mpn: String,
}

impl AssemblyRuleRecord {
    pub fn build_assembly_rule(&self) -> Result<AssemblyRule, anyhow::Error> {
        Ok(AssemblyRule {
            ref_des: self.ref_des.clone(),
            manufacturer: self.manufacturer.clone(),
            mpn: self.mpn.clone(),
        })
    }
}
