use crate::pnp::part::Part;
use crate::pnp::placement::Placement;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlacementRecord {
    pub ref_des: String,
    pub manufacturer: String,
    pub mpn: String,
    pub place: bool,
}

impl PlacementRecord {
    pub fn as_placement(&self) -> Placement {
        Placement {
            ref_des: self.ref_des.clone(),
            part: Part { manufacturer: self.manufacturer.clone(), mpn: self.mpn.clone() },
            place: self.place,
        }
    }
}