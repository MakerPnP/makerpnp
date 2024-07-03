mod criteria;
mod part_mapping;

use crate::part::Part;
use crate::part_mapper::part_mapping::PartMapping;

pub struct PartMapper {}

impl PartMapper {
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
    use crate::part_mapper::part_mapping::PartMapping;
    use crate::part_mapper::criteria::ExactMatchCriteria;
    use crate::part_mapper::PartMapper;

    #[test]
    fn map_parts() {

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
        let matched_mappings = PartMapper::process(parts, part_mappings);

        assert_eq!(matched_mappings, expected_part_mappings);
    }
}