use crate::pnp::part::Part;

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

pub fn find_load_out_item_by_part<'load_out>(load_out_items: &'load_out [LoadOutItem], part: &Part) -> Option<&'load_out LoadOutItem> {
    let matched_item = load_out_items.iter().find(|&load_out_item| {
        load_out_item.manufacturer.eq(&part.manufacturer)
            && load_out_item.mpn.eq(&part.mpn)
    });
    matched_item
}