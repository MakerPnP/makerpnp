use crate::part::Part;

pub struct PartMapping<'part, 'criteria>
{
    part: &'part Part,
    criteria: Vec<&'criteria dyn PartMappingCriteria>,
}

impl<'part, 'criteria> PartMapping<'part, 'criteria> {
    pub fn new(part: &'part Part, criteria: Vec<&'criteria dyn PartMappingCriteria>) -> Self {
        Self {
            part,
            criteria
        }
    }
}

pub trait PartMappingCriteria {
    fn matches(&self, part: &Part) -> bool;
}

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
        part_mappings: Vec<&'mapping PartMapping<'mapping, 'mapping>>
    ) -> Option<Vec<&'mapping PartMapping<'mapping, 'mapping>>> {

        let mut results: Vec<&'mapping PartMapping<'mapping, 'mapping>> = vec![];

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
        let part_mapping1 = PartMapping::new(&part1, vec![&criteria1]);
        let criteria2 = ExactMatchCriteria::new("MFR2".to_string(), "PART2".to_string());
        let part_mapping2 = PartMapping::new(&part2, vec![&criteria2]);
        let criteria3 = ExactMatchCriteria::new("MFR3".to_string(), "PART3".to_string());
        let part_mapping3 = PartMapping::new(&part3, vec![&criteria3]);

        let part_mappings = vec![&part_mapping1, &part_mapping2, &part_mapping3];

        // and
        let expected_part_mappings = Some(vec![
            &part_mapping1,
            &part_mapping2,
            &part_mapping3,
        ]);

        // when
        let matched_mappings = PartMatcher::process(parts, part_mappings);

        // then
        assert!(matched_mappings.is_some());
        for (matched_mapping, expected_mapping) in matched_mappings.unwrap().iter().zip(expected_part_mappings.unwrap().iter()) {
            println!("{:?}", matched_mapping.part);
            assert_eq!(matched_mapping.part, expected_mapping.part);
        }
        // FIXME we just want to do this, but cannot, due to the type specification of criteria in PartMapping
        //assert_eq!(matched_mappings, expected_part_mappings);
    }
}