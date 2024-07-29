
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlacementRecord {
    pub ref_des: String,
    pub manufacturer: String,
    pub mpn: String,
    pub place: bool,
}