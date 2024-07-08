#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub struct EdaPlacement {
    pub ref_des: String,
    pub details: EdaPlacementDetails,
}

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub enum EdaPlacementDetails {
    DipTrace(DipTracePlacementDetails)
    //...KiCad(KiCadPlacementDetails)
}

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub struct DipTracePlacementDetails {
    pub name: String,
    pub value: String,
}