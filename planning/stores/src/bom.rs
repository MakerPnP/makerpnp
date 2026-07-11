#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BOMRecord {
    pub ref_des_set: String,
    pub manufacturer: String,
    pub mpn: String,
    pub quantity: usize,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct JLCPCBBOMRecord {
    pub comment: String,
    pub designator: String,
    pub footprint: String,

    #[serde(default)]
    pub jlcpcb_part: Option<String>,

    pub quantity: usize,
}
