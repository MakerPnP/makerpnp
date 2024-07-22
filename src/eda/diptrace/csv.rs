use thiserror::Error;
use crate::eda::placement::{EdaPlacement, EdaPlacementField};

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
            fields: vec![
                EdaPlacementField { name: "name".to_string(), value: self.name.to_string() },
                EdaPlacementField { name: "value".to_string(), value: self.value.to_string() },
            ],
        })

        // _ => Err(DiptracePlacementRecordError::Unknown)
    }
}
