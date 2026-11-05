#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BOMRecord {
    pub ref_des_set: String,
    pub manufacturer: String,
    pub mpn: String,
    pub quantity: usize,
}

/// The names of the columns of a [`JLCPCBBOMRecord`], in order, as a BOM is written without headers, so they can be
/// shown.
pub const JLCPCB_BOM_COLUMNS: [&str; 5] = ["Comment", "Designator", "Footprint", "JLCPCB Part", "Quantity"];

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
