use thiserror::Error;
use crate::eda::eda_placement::{EdaPlacement, EdaPlacementField};

#[derive(Error, Debug)]
pub enum KiCadPlacementRecordError {
    #[error("Unknown")]
    Unknown
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct KiCadPlacementRecord {
    #[serde(rename(deserialize = "ref"))]
    ref_des: String,
    package: String,
    val: String,
}

impl KiCadPlacementRecord {
    pub fn build_eda_placement(&self) -> Result<EdaPlacement, KiCadPlacementRecordError> {
        Ok(EdaPlacement {
            ref_des: self.ref_des.to_string(),
            place: true,
            fields: vec![
                EdaPlacementField { name: "package".to_string(), value: self.package.to_string() },
                EdaPlacementField { name: "val".to_string(), value: self.val.to_string() },
            ],
        })

        // _ => Err(KiCadPlacementRecordError::Unknown)
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct KiCadSubstitutionRecord {
    // from
    pub package_pattern: String,
    pub val_pattern: String,

    // to
    pub package: String,
    pub val: String,
}
