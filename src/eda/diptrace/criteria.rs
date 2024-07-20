use crate::eda::eda_placement::{EdaPlacement, EdaPlacementDetails};
use crate::part_mapper::criteria::PlacementMappingCriteria;
#[derive(Debug, PartialEq)]
pub struct DipTraceExactMatchCriteria {
    name: String,
    value: String,
}

impl PlacementMappingCriteria for DipTraceExactMatchCriteria {
    fn matches(&self, placement: &EdaPlacement) -> bool {
        match &placement.details {
            EdaPlacementDetails::DipTrace(details) => {
                self.name.eq(&details.name) && self.value.eq(&details.value)
            },
            // _ => false
        }
    }
}

#[cfg(test)]
mod exact_match_critera_tests {
    use crate::part_mapper::criteria::PlacementMappingCriteria;
    use crate::eda::diptrace::criteria::DipTraceExactMatchCriteria;
    use crate::eda::eda_placement::{DipTracePlacementDetails, EdaPlacement};
    use crate::eda::eda_placement::EdaPlacementDetails::DipTrace;

    #[test]
    fn matches() {
        // given
        let criteria = DipTraceExactMatchCriteria::new("NAME1".to_string(), "VALUE1".to_string());
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE1".to_string() }),
        };

        // when
        assert!(criteria.matches(&placement));
    }

    #[test]
    fn does_not_match_due_to_name() {
        // given
        let criteria = DipTraceExactMatchCriteria::new("NAME1".to_string(), "VALUE1".to_string());
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME2".to_string(), value: "VALUE1".to_string() }),
        };

        // when
        assert!(!criteria.matches(&placement));
    }

    #[test]
    fn does_not_match_due_to_value() {
        // given
        let criteria = DipTraceExactMatchCriteria::new("NAME1".to_string(), "VALUE1".to_string());
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE2".to_string() }),
        };

        // when
        assert!(!criteria.matches(&placement));
    }
}

impl DipTraceExactMatchCriteria {
    pub fn new(name: String, value: String) -> Self {
        Self {
            name,
            value
        }
    }
}