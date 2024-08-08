use csv::QuoteStyle;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all(serialize = "PascalCase"))]
pub struct TestPhasePlacementRecord {
    pub object_path: String,
    pub feeder_reference: String,
    pub manufacturer: String,
    pub mpn: String,
}

#[derive(Default)]
pub struct PhasePlacementsCSVBuilder<'a> {
    records: Option<&'a [TestPhasePlacementRecord]>
}

impl<'a> PhasePlacementsCSVBuilder<'a> {
    
    pub fn as_string(&mut self) -> String {
        let content: Vec<u8> = vec![];

        let mut writer = csv::WriterBuilder::new()
            .quote_style(QuoteStyle::Always)
            .from_writer(content);

        if let Some(records) = self.records {
            for record in records.iter() {
                writer.serialize(record).unwrap();
            }
        }
        
        writer.flush().unwrap();
                
        String::from_utf8(writer.into_inner().unwrap()).unwrap()
    }
    pub fn with_items(mut self, records: &'a [TestPhasePlacementRecord]) -> Self {
        self.records = Some(records);
        self
    }
    
    pub fn new() -> Self {
        Default::default()
    }
}
