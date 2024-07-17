use thiserror::Error;
use crate::eda::eda_placement::{DipTracePlacementDetails, EdaPlacement, EdaPlacementDetails};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct DipTracePartMappingRecord {
    // from
    pub name: String,
    pub value: String,

    // to
    pub manufacturer: String,
    pub mpn: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct DiptracePlacementRecord {
    ref_des: String,
    name: String,
    value: String,
}

#[derive(Error, Debug)]
pub enum DiptracePlacementRecordError {
    #[error("Unknown")]
    Unknown
}

impl DiptracePlacementRecord {
    pub fn build_eda_placement(&self) -> Result<EdaPlacement, DiptracePlacementRecordError> {
        Ok(EdaPlacement {
            ref_des: self.ref_des.to_string(),
            place: true,
            details: EdaPlacementDetails::DipTrace(DipTracePlacementDetails {
                name: self.name.to_string(),
                value: self.value.to_string(),
            })
        })

        // _ => Err(DiptracePlacementRecordError::Unknown)
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct DipTraceSubstitutionRecord {
    // from
    pub name_pattern: String,
    pub value_pattern: String,

    // to
    pub name: String,
    pub value: String,
}
