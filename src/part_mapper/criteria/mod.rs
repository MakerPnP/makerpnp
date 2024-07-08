use std::fmt::Debug;
use crate::eda::eda_placement::EdaPlacement;
use crate::util::dynamic::as_any::AsAny;
use crate::util::dynamic::dynamic_eq::DynamicEq;

pub trait PlacementMappingCriteria: Debug + AsAny + DynamicEq {
    fn matches(&self, placement: &EdaPlacement) -> bool;
}

impl PartialEq for dyn PlacementMappingCriteria
{
    fn eq(&self, other: &Self) -> bool {
        self.dynamic_eq(other.as_any())
    }
}

