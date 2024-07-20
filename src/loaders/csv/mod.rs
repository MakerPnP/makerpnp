use thiserror::Error;
use crate::assembly::rules::AssemblyRule;
use crate::eda::diptrace::csv::{DipTracePartMappingRecord, DipTraceSubstitutionRecord};
use crate::pnp::part::Part;
use crate::eda::diptrace::criteria::DipTraceExactMatchCriteria;
use crate::eda::eda_substitution::{DipTraceSubstitutionRuleDetails, EdaSubstitutionRule, EdaSubstitutionRuleDetails};
use crate::part_mapper::criteria::PlacementMappingCriteria;
use crate::part_mapper::part_mapping::PartMapping;
use crate::pnp::load_out_item::LoadOutItem;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct CSVPartMappingRecord {
    eda: String,
    name: String,
    value: String,
    manufacturer: String,
    mpn: String,
}

#[non_exhaustive]
pub enum PartMappingRecord {
    DipTracePartMapping(DipTracePartMappingRecord),
    // TODO add KiCad support
    //KiCadPartMapping(KiCadPartMappingRecord),
}

#[derive(Error, Debug)]
pub enum CSVPartMappingRecordError {
    #[error("Unknown EDA: '{eda:?}'")]
    UnknownEDA { eda: String }
}

impl TryFrom<CSVPartMappingRecord> for PartMappingRecord {
    type Error = CSVPartMappingRecordError;

    fn try_from(value: CSVPartMappingRecord) -> Result<Self, Self::Error> {
        match value.eda.as_str() {
            "DipTrace" => Ok(PartMappingRecord::DipTracePartMapping(DipTracePartMappingRecord {
                name: value.name.to_string(),
                value: value.value.to_string(),
                manufacturer: value.manufacturer.to_string(),
                mpn: value.mpn.to_string(),
            })),
            _ => Err(CSVPartMappingRecordError::UnknownEDA { eda: value.eda }),
        }
    }
}

#[derive(Error, Debug)]
pub enum PartMappingRecordError {
    #[error("Unable to build criteria")]
    UnableToBuildCriteria,

    #[error("No matching part, criteria: {criteria:?}")]
    NoMatchingPart { criteria: Part },
}

impl PartMappingRecord {
    pub fn build_part_mapping<'part>(&self, parts: &'part Vec<Part>) -> Result<PartMapping<'part>, PartMappingRecordError> {

        let part_criteria: Part = match self {
            PartMappingRecord::DipTracePartMapping(r) => Ok(Part { manufacturer: r.manufacturer.clone(), mpn: r.mpn.clone() }),
            // TODO add KiCad support
            // _ => Err(PartMappingError::UnableToBuildCriteria)
        }?;

        let matched_part_ref = parts.iter().find_map(|part| {
            match part.eq(&part_criteria) {
                true => Some(part),
                false => None,
            }
        });

        let part_ref = match matched_part_ref {
            Some(part) => Ok(part),
            _ => Err(PartMappingRecordError::NoMatchingPart { criteria: part_criteria })
        }?;

        let criterion = match self {
            PartMappingRecord::DipTracePartMapping(record) => {
                Ok(DipTraceExactMatchCriteria::new(record.name.clone(), record.value.clone()))
            }
        }?;


        let criteria: Vec<Box<dyn PlacementMappingCriteria>> = vec![Box::new(criterion)];

        let part_mapping = PartMapping::new(part_ref, criteria);

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
    pub fn build_part(&self) -> Result<Part, ()> {
        Ok(Part {
            manufacturer: self.manufacturer.clone(),
            mpn: self.mpn.clone(),
        })
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct LoadOutItemRecord {
    reference: String,
    manufacturer: String,
    mpn: String,
}

impl LoadOutItemRecord {
    pub fn build_load_out_item(&self) -> Result<LoadOutItem, ()> {
        Ok(LoadOutItem {
            reference: self.reference.clone(),
            manufacturer: self.manufacturer.clone(),
            mpn: self.mpn.clone(),
        })
    }
}


#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct CSVSubstitutionRecord {
    eda: String,
    name_pattern: String,
    value_pattern: String,
    name: String,
    value: String,
}

#[non_exhaustive]
pub enum SubstitutionRecord {
    DipTraceSubstitution(DipTraceSubstitutionRecord),
    // TODO add KiCad support
    //KiCadSubstitution(KiCadSubstitutionRecord),
}

impl SubstitutionRecord {
    pub fn build_eda_substitution(&self) -> Result<EdaSubstitutionRule, ()> {
        match self {
            SubstitutionRecord::DipTraceSubstitution(record) => Ok(EdaSubstitutionRule {
                details: EdaSubstitutionRuleDetails::DipTrace(DipTraceSubstitutionRuleDetails {
                    name_pattern: record.name_pattern.to_string(),
                    value_pattern: record.value_pattern.to_string(),
                    name: record.name.to_string(),
                    value: record.value.to_string(),
                }),
            }),
        }
    }
}

#[derive(Error, Debug)]
pub enum CSVSubstitutionRecordError {
    #[error("Unknown EDA: '{eda:?}'")]
    UnknownEDA { eda: String }
}

impl TryFrom<CSVSubstitutionRecord> for SubstitutionRecord {
    type Error = CSVSubstitutionRecordError;

    fn try_from(value: CSVSubstitutionRecord) -> Result<Self, Self::Error> {
        match value.eda.as_str() {
            "DipTrace" => Ok(SubstitutionRecord::DipTraceSubstitution(DipTraceSubstitutionRecord {
                name_pattern: value.name_pattern.to_string(),
                value_pattern: value.value_pattern.to_string(),
                name: value.name.to_string(),
                value: value.value.to_string(),
            })),
            _ => Err(CSVSubstitutionRecordError::UnknownEDA { eda: value.eda }),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct AssemblyRuleRecord {
    ref_des: String,
    manufacturer: String,
    mpn: String,
}

impl AssemblyRuleRecord {
    pub fn build_assembly_rule(&self) -> Result<AssemblyRule, ()> {
        Ok(AssemblyRule {
            ref_des: self.ref_des.clone(),
            manufacturer: self.manufacturer.clone(),
            mpn: self.mpn.clone(),
        })
    }
}
