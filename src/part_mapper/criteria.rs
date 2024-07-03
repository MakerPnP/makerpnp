use std::fmt::Debug;
use crate::util::dynamic::as_any::AsAny;
use crate::util::dynamic::dynamic_eq::DynamicEq;
use crate::part::Part;

pub trait PartMappingCriteria : Debug + AsAny + DynamicEq {
    fn matches(&self, part: &Part) -> bool;
}

impl PartialEq for dyn PartMappingCriteria
{
    fn eq(&self, other: &Self) -> bool {
        self.dynamic_eq(other.as_any())
    }
}

#[derive(Debug, PartialEq)]
pub struct ExactMatchCriteria {
    manufacturer: String,
    mpn: String,
}

impl PartMappingCriteria for ExactMatchCriteria {
    fn matches(&self, part: &Part) -> bool {
        self.manufacturer.eq(&part.manufacturer) && self.mpn.eq(&part.mpn)
    }
}

#[cfg(test)]
mod exact_match_critera_tests {
    use crate::part::Part;
    use super::*;

    #[test]
    fn matches() {
        // given
        let criteria = ExactMatchCriteria::new("MFR1".to_string(), "PART1".to_string());
        let part = Part::new("MFR1".to_string(), "PART1".to_string());

        // when
        assert!(criteria.matches(&part));
    }

    #[test]
    fn does_not_match_due_to_manufacturer_difference() {
        // given
        let criteria = ExactMatchCriteria::new("MFR1".to_string(), "PART1".to_string());
        let part = Part::new("MFR2".to_string(), "PART1".to_string());

        // when
        assert!(!criteria.matches(&part));
    }

    #[test]
    fn does_not_match_due_to_mpn_difference() {
        // given
        let criteria = ExactMatchCriteria::new("MFR1".to_string(), "PART1".to_string());
        let part = Part::new("MFR1".to_string(), "PART2".to_string());

        // when
        assert!(!criteria.matches(&part));
    }
}

impl ExactMatchCriteria {
    pub fn new(manufacturer: String, mpn: String) -> Self {
        Self {
            manufacturer,
            mpn,
        }
    }
}
