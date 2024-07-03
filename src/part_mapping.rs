use std::fmt::Debug;
use crate::as_any::AsAny;
use crate::dyn_eq::DynEq;
use crate::part::Part;


#[derive(Debug, PartialEq)]
pub struct PartMapping<'part>
{
    part: &'part Part,
    criteria: Vec<Box<dyn PartMappingCriteria>>,
}

impl<'part> PartMapping<'part> {
    pub fn new(part: &'part Part, criteria: Vec<Box<dyn PartMappingCriteria>>) -> Self {
        Self {
            part,
            criteria
        }
    }
}

pub trait PartMappingCriteria : Debug + AsAny + DynEq {
    fn matches(&self, part: &Part) -> bool;
}

impl PartialEq for dyn PartMappingCriteria
{
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other.as_any())
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
    use crate::part_mapping::{ExactMatchCriteria, PartMappingCriteria};

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

struct PartMatcher {}

impl PartMatcher {
    pub fn process<'parts, 'mapping>(
        parts: Vec<&'parts Part>,
        part_mappings: Vec<&'mapping PartMapping<'mapping>>
    ) -> Option<Vec<&'mapping PartMapping<'mapping>>> {

        let mut results: Vec<&'mapping PartMapping<'mapping>> = vec![];

        for part in parts.iter() {
            for part_mapping in part_mappings.iter() {
                for criteria in part_mapping.criteria.iter() {
                    if criteria.matches(part) {
                        results.push(part_mapping);
                    }
                }
            }
        }

        match results.is_empty() {
            true => None,
            false => Some(results)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::part::Part;
    use crate::part_mapping::{ExactMatchCriteria, PartMapping, PartMatcher};

    #[test]
    fn match_parts() {

        // given
        let part1 = Part::new("MFR1".to_string(), "PART1".to_string());
        let part2 = Part::new("MFR2".to_string(), "PART2".to_string());
        let part3 = Part::new("MFR3".to_string(), "PART3".to_string());

        let parts = vec![&part1, &part2, &part3];

        // and
        let criteria1 = ExactMatchCriteria::new("MFR1".to_string(), "PART1".to_string());
        let part_mapping1 = PartMapping::new(&part1, vec![Box::new(criteria1)]);
        let criteria2 = ExactMatchCriteria::new("MFR2".to_string(), "PART2".to_string());
        let part_mapping2 = PartMapping::new(&part2, vec![Box::new(criteria2)]);
        let criteria3 = ExactMatchCriteria::new("MFR3".to_string(), "PART3".to_string());
        let part_mapping3 = PartMapping::new(&part3, vec![Box::new(criteria3)]);

        let part_mappings = vec![&part_mapping1, &part_mapping2, &part_mapping3];

        // and
        let expected_part_mappings = Some(vec![
            &part_mapping1,
            &part_mapping2,
            &part_mapping3,
        ]);

        // when
        let matched_mappings = PartMatcher::process(parts, part_mappings);

        assert_eq!(matched_mappings, expected_part_mappings);
    }
}