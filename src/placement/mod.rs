#[cfg_attr(test, derive(PartialEq, Debug))]
#[derive(Clone)]
pub struct Placement {
    pub ref_des: String,
}

impl Placement {
    pub fn new(ref_des: String) -> Self {
        Self {
            ref_des
        }
    }
}

pub mod eda {
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
}
