use crate::eda::eda_placement::{EdaPlacement, EdaPlacementDetails};
use crate::part_mapper::criteria::PlacementMappingCriteria;
#[derive(Debug, PartialEq)]
pub struct KiCadExactMatchCriteria {
    package: String,
    val: String,

    // TODO consider using a hashmap of fields and values so it can work for any type of placement
    //      then move outside of the KiCad package
}

impl PlacementMappingCriteria for KiCadExactMatchCriteria {
    fn matches(&self, placement: &EdaPlacement) -> bool {
        match &placement.details {
            EdaPlacementDetails::KiCad(details) => {
                self.package.eq(&details.package) && self.val.eq(&details.val)
            },
            _ => false
        }
    }
}

#[cfg(test)]
mod exact_match_critera_tests {
    use crate::part_mapper::criteria::PlacementMappingCriteria;
    use crate::eda::kicad::criteria::KiCadExactMatchCriteria;
    use crate::eda::eda_placement::{KiCadPlacementDetails, EdaPlacement};
    use crate::eda::eda_placement::EdaPlacementDetails::KiCad;

    #[test]
    fn matches() {
        // given
        let criteria = KiCadExactMatchCriteria::new("PACKAGE1".to_string(), "VAL1".to_string());
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            details: KiCad(KiCadPlacementDetails { package: "PACKAGE1".to_string(), val: "VAL1".to_string() }),
        };

        // when
        assert!(criteria.matches(&placement));
    }

    #[test]
    fn does_not_match_due_to_package() {
        // given
        let criteria = KiCadExactMatchCriteria::new("PACKAGE1".to_string(), "VAL1".to_string());
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            details: KiCad(KiCadPlacementDetails { package: "PACKAGE2".to_string(), val: "VAL1".to_string() }),
        };

        // when
        assert!(!criteria.matches(&placement));
    }

    #[test]
    fn does_not_match_due_to_val() {
        // given
        let criteria = KiCadExactMatchCriteria::new("PACKAGE1".to_string(), "VAL1".to_string());
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            details: KiCad(KiCadPlacementDetails { package: "PACKAGE1".to_string(), val: "VAL2".to_string() }),
        };

        // when
        assert!(!criteria.matches(&placement));
    }
}

impl KiCadExactMatchCriteria {
    pub fn new(package: String, val: String) -> Self {
        Self {
            package: package,
            val: val
        }
    }
}