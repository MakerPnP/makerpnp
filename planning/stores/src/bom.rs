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

/// A BOM's records, written without headers, as JLCPCB's are, so each is read by the order of its values (`STORE-17`).
pub fn read_jlcpcb_bom(reader: impl std::io::Read) -> Result<Vec<JLCPCBBOMRecord>, anyhow::Error> {
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(reader);

    csv_reader
        .deserialize()
        .map(|record| record.map_err(anyhow::Error::from))
        .collect()
}
