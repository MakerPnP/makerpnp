use thiserror::Error;
use crate::assembly::rules::AssemblyRule;
use crate::eda::criteria::{GenericCriteriaItem, GenericExactMatchCriteria};
use crate::eda::diptrace::csv::{DipTracePartMappingRecord, DipTraceSubstitutionRecord, KiCadPartMappingRecord};
use crate::pnp::part::Part;
use crate::eda::eda_substitution::{EdaSubstitutionRule, EdaSubstitutionRuleTransformItem, EdaSubstitutionRuleCriteriaItem};
use crate::eda::kicad::csv::KiCadSubstitutionRecord;
use crate::part_mapper::criteria::PlacementMappingCriteria;
use crate::part_mapper::part_mapping::PartMapping;
use crate::pnp::load_out_item::LoadOutItem;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct CSVPartMappingRecord {
    eda: String,
    // DipTrace
    name: Option<String>,
    value: Option<String>,
    // KiCad
    package: Option<String>,
    val: Option<String>,

    manufacturer: String,
    mpn: String,
}

#[non_exhaustive]
pub enum PartMappingRecord {
    DipTracePartMapping(DipTracePartMappingRecord),
    KiCadPartMapping(KiCadPartMappingRecord),
}

#[derive(Error, Debug)]
pub enum CSVPartMappingRecordError {
    #[error("Unknown EDA: '{eda:?}'")]
    UnknownEDA { eda: String }
}

impl TryFrom<CSVPartMappingRecord> for PartMappingRecord {
    type Error = CSVPartMappingRecordError;

    fn try_from(value: CSVPartMappingRecord) -> Result<Self, Self::Error> {
        // FIXME unwrap() might fail below if the CSV file columns don't exist.
        match value.eda.as_str() {
            "DipTrace" => Ok(PartMappingRecord::DipTracePartMapping(DipTracePartMappingRecord {
                name: value.name.unwrap().to_string(),
                value: value.value.unwrap().to_string(),

                manufacturer: value.manufacturer.to_string(),
                mpn: value.mpn.to_string(),
            })),
            "KiCad" => Ok(PartMappingRecord::KiCadPartMapping(KiCadPartMappingRecord {
                package: value.package.unwrap().to_string(),
                val: value.val.unwrap().to_string(),

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
            PartMappingRecord::KiCadPartMapping(r) => Ok(Part { manufacturer: r.manufacturer.clone(), mpn: r.mpn.clone() }),
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

        let mut mapping_criteria: Vec<Box<dyn PlacementMappingCriteria>> = vec![];

        match self {
            PartMappingRecord::DipTracePartMapping(record) => {
                let criteria = GenericExactMatchCriteria { criteria: vec![
                    GenericCriteriaItem::new("name".to_string(), record.name.clone()),
                    GenericCriteriaItem::new("value".to_string(), record.value.clone()),
                ]};

                mapping_criteria.push(Box::new(criteria))
            }
            PartMappingRecord::KiCadPartMapping(record) => {
                let criteria = GenericExactMatchCriteria { criteria: vec![
                    GenericCriteriaItem::new("package".to_string(), record.package.clone()),
                    GenericCriteriaItem::new("val".to_string(), record.val.clone()),
                ]};

                mapping_criteria.push(Box::new(criteria))
            },
            // TODO investigate using non_exhaustive on the PartMappingRecord
            //_ => return Err(UnableToBuildCriteria)
        };

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

    // DipTrace From
    name_pattern: Option<String>,
    value_pattern: Option<String>,
    // KiCad From
    package_pattern: Option<String>,
    val_pattern: Option<String>,

    // DipTrace To
    name: Option<String>,
    value: Option<String>,
    // KiCad To
    package: Option<String>,
    val: Option<String>,
}

#[non_exhaustive]
pub enum SubstitutionRecord {
    DipTraceSubstitution(DipTraceSubstitutionRecord),
    KiCadSubstitution(KiCadSubstitutionRecord),
}

impl SubstitutionRecord {
    pub fn build_eda_substitution(&self) -> Result<EdaSubstitutionRule, ()> {
        match self {
            SubstitutionRecord::DipTraceSubstitution(record) => {
                let mut criteria: Vec<EdaSubstitutionRuleCriteriaItem> = vec![];
                criteria.push(EdaSubstitutionRuleCriteriaItem { field_name: "name".to_string(), field_pattern: record.name_pattern.clone() } );
                criteria.push(EdaSubstitutionRuleCriteriaItem { field_name: "value".to_string(), field_pattern: record.value_pattern.clone() } );

                let mut transforms: Vec<EdaSubstitutionRuleTransformItem> = vec![];
                transforms.push(EdaSubstitutionRuleTransformItem { field_name: "name".to_string(), field_value: record.name.clone() } );
                transforms.push(EdaSubstitutionRuleTransformItem { field_name: "value".to_string(), field_value: record.value.clone() } );

                Ok(EdaSubstitutionRule { criteria, transforms })
            },
            SubstitutionRecord::KiCadSubstitution(record) => {
                let mut criteria: Vec<EdaSubstitutionRuleCriteriaItem> = vec![];
                criteria.push(EdaSubstitutionRuleCriteriaItem { field_name: "package".to_string(), field_pattern: record.package_pattern.clone() } );
                criteria.push(EdaSubstitutionRuleCriteriaItem { field_name: "val".to_string(), field_pattern: record.val_pattern.clone() } );

                let mut transforms: Vec<EdaSubstitutionRuleTransformItem> = vec![];
                transforms.push(EdaSubstitutionRuleTransformItem { field_name: "package".to_string(), field_value: record.package.clone() } );
                transforms.push(EdaSubstitutionRuleTransformItem { field_name: "val".to_string(), field_value: record.val.clone() } );

                Ok(EdaSubstitutionRule { criteria, transforms })
            },
        }
    }
}

#[derive(Error, Debug)]
pub enum CSVSubstitutionRecordError {
    #[error("Unknown EDA: '{eda:}'")]
    UnknownEDA { eda: String }
}

impl TryFrom<CSVSubstitutionRecord> for SubstitutionRecord {
    type Error = CSVSubstitutionRecordError;

    fn try_from(value: CSVSubstitutionRecord) -> Result<Self, Self::Error> {
        // FIXME unwrap() might fail below if the CSV file columns don't exist.
        match value.eda.as_str() {
            "DipTrace" => Ok(SubstitutionRecord::DipTraceSubstitution(DipTraceSubstitutionRecord {
                name_pattern: value.name_pattern.unwrap().to_string(),
                value_pattern: value.value_pattern.unwrap().to_string(),
                name: value.name.unwrap().to_string(),
                value: value.value.unwrap().to_string(),
            })),
            "KiCad" => Ok(SubstitutionRecord::KiCadSubstitution(KiCadSubstitutionRecord {
                package_pattern: value.package_pattern.unwrap().to_string(),
                val_pattern: value.val_pattern.unwrap().to_string(),
                package: value.package.unwrap().to_string(),
                val: value.val.unwrap().to_string(),
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
