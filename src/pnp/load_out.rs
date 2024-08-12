#[derive(Debug, PartialEq)]
pub struct LoadOutItem {
    pub reference: String,
    pub manufacturer: String,
    pub mpn: String,
}

impl LoadOutItem {
    pub fn new(reference: String, manufacturer: String, mpn: String) -> Self {
        Self {
            reference,
            manufacturer,
            mpn,
        }
    }
}
