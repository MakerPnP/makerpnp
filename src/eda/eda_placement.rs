#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaPlacement {
    pub ref_des: String,
    pub place: bool,
    pub details: EdaPlacementDetails,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EdaPlacementDetails {
    DipTrace(DipTracePlacementDetails),
    KiCad(KiCadPlacementDetails),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DipTracePlacementDetails {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KiCadPlacementDetails {
    pub package: String,
    pub val: String,
}

