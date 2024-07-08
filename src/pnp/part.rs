#[derive(Debug, PartialEq)]
pub struct Part {
    pub manufacturer: String,
    pub mpn: String,
}

impl Part {
    pub fn new(manufacturer: String, mpn: String) -> Self {
        Self {
            manufacturer,
            mpn,
        }
    }
}

