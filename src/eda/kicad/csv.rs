use thiserror::Error;
use crate::eda::placement::{EdaPlacement, EdaPlacementField};
use crate::planning::PcbSide;

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
    side: KiCadPcbSide,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all(deserialize = "lowercase"))]
enum KiCadPcbSide {
    Top,
    Bottom,
}

impl From<&KiCadPcbSide> for PcbSide {
    fn from(value: &KiCadPcbSide) -> Self {
        match value {
            KiCadPcbSide::Top => PcbSide::Top,
            KiCadPcbSide::Bottom => PcbSide::Bottom,
        }
    }
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
            pcb_side: PcbSide::from(&self.side),
        })

        // _ => Err(KiCadPlacementRecordError::Unknown)
    }
}
