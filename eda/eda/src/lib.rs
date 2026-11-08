pub mod diptrace;
pub mod easyeda;
pub mod kicad;

pub mod criteria;
pub mod placement;
pub mod substitution;

#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord
)]
pub enum EdaTool {
    DipTrace,
    KiCad,
    EasyEda,
}

impl EdaTool {
    /// Every EDA tool, in order.
    pub const ALL: [EdaTool; 3] = [EdaTool::DipTrace, EdaTool::KiCad, EdaTool::EasyEda];

    /// The names of the fields of the tool's placements that part mappings and substitutions match, in order, as its
    /// placements files name them.
    pub fn field_names(&self) -> &'static [&'static str] {
        match self {
            EdaTool::DipTrace => &["name", "value"],
            EdaTool::KiCad => &["package", "val"],
            EdaTool::EasyEda => &["device", "value"],
        }
    }

    /// A placement's fields, named as [`Self::field_names`] names them, with a value for each, in the same order.
    pub fn placement_fields(&self, values: &[&str]) -> Vec<placement::EdaPlacementField> {
        let names = self.field_names();
        assert_eq!(names.len(), values.len(), "a value for each of the tool's fields");
        names
            .iter()
            .zip(values)
            .map(|(name, value)| placement::EdaPlacementField::new(name.to_string(), value.to_string()))
            .collect()
    }

    /// Every tool's field names, in the tools' order, each once, as a field, such as `value`, can be more than one
    /// tool's.
    pub fn all_field_names() -> Vec<&'static str> {
        let mut names = vec![];
        for name in Self::ALL
            .iter()
            .flat_map(|tool| tool.field_names())
        {
            if !names.contains(name) {
                names.push(*name);
            }
        }
        names
    }
}

#[cfg(test)]
mod field_names_tests {
    use crate::EdaTool;

    #[test]
    fn every_tools_field_names_are_each_given_once_in_the_tools_order() {
        assert_eq!(EdaTool::all_field_names(), vec![
            "name", "value", "package", "val", "device"
        ]);
    }
}
