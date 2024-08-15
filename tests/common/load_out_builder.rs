use csv::QuoteStyle;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all(serialize = "PascalCase"))]
pub struct TestLoadOutRecord {
    pub reference: String,
    pub manufacturer: String,
    pub mpn: String,
}

#[derive(Default)]
pub struct LoadOutCSVBuilder<'a> {
    records: Option<&'a [TestLoadOutRecord]>
}

impl<'a> LoadOutCSVBuilder<'a> {
    
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
    pub fn with_items(mut self, records: &'a [TestLoadOutRecord]) -> Self {
        self.records = Some(records);
        self
    }
    
    pub fn new() -> Self {
        Default::default()
    }
    
    
}
