
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaPlacementField {
    pub name: String,
    // FUTURE if there's a requirement to store other EDA specific data types other than String, perhaps implement an emum named EdaPlacementValue.
    pub value: String,
}

impl EdaPlacementField {
    pub fn new(name: String, value: String) -> Self {
        Self {
            name,
            value,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaPlacement {
    pub ref_des: String,
    pub place: bool,
    pub fields: Vec<EdaPlacementField>,
}
