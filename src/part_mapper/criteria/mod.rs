use std::fmt::Debug;
use crate::placement::eda::EdaPlacement;
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

pub mod diptrace {
    use crate::part_mapper::criteria::PlacementMappingCriteria;
    use crate::placement::eda::{EdaPlacement, EdaPlacementDetails};

    #[derive(Debug, PartialEq)]
    pub struct ExactMatchCriteria {
        name: String,
        value: String,
    }

    impl PlacementMappingCriteria for ExactMatchCriteria {
        fn matches(&self, placement: &EdaPlacement) -> bool {
            match &placement.details {
                EdaPlacementDetails::DipTrace(details) => {
                    self.name.eq(&details.name) && self.value.eq(&details.value)
                },
                _ => false
            }
        }
    }

    #[cfg(test)]
    mod exact_match_critera_tests {
        use crate::placement::eda::DipTracePlacementDetails;
        use crate::placement::eda::EdaPlacementDetails::DipTrace;
        use super::*;

        #[test]
        fn matches() {
            // given
            let criteria = ExactMatchCriteria::new("NAME1".to_string(), "VALUE1".to_string());
            let placement = EdaPlacement {
                ref_des: "R1".to_string(),
                details: DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE1".to_string() }),
            };

            // when
            assert!(criteria.matches(&placement));
        }

        #[test]
        fn does_not_match_due_to_name() {
            // given
            let criteria = ExactMatchCriteria::new("NAME1".to_string(), "VALUE1".to_string());
            let placement = EdaPlacement {
                ref_des: "R1".to_string(),
                details: DipTrace(DipTracePlacementDetails { name: "NAME2".to_string(), value: "VALUE1".to_string() }),
            };

            // when
            assert!(!criteria.matches(&placement));
        }

        #[test]
        fn does_not_match_due_to_value() {
            // given
            let criteria = ExactMatchCriteria::new("NAME1".to_string(), "VALUE1".to_string());
            let placement = EdaPlacement {
                ref_des: "R1".to_string(),
                details: DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE2".to_string() }),
            };

            // when
            assert!(!criteria.matches(&placement));
        }
    }

    impl ExactMatchCriteria {
        pub fn new(name: String, value: String) -> Self {
            Self {
                name,
                value
            }
        }
    }
}