pub mod criteria;
pub mod part_mapping;

use crate::eda::eda_placement::EdaPlacement;
use crate::part_mapper::part_mapping::PartMapping;

pub struct PartMapper {}

impl PartMapper {
    pub fn process<'placement, 'mapping>(
        eda_placements: &'placement Vec<EdaPlacement>,
        part_mappings: &'mapping Vec<PartMapping<'mapping>>
    ) -> Vec<ProcessingResult<'placement, 'mapping>> {

        let mut results = vec![];

        for eda_placement in eda_placements.iter() {
            let mut matching_mappings = vec![];

            for part_mapping in part_mappings.iter() {
                for criteria in part_mapping.criteria.iter() {
                    if criteria.matches(eda_placement) {
                        matching_mappings.push(part_mapping);
                    }
                }
            }

            let matching_mappings_result = match matching_mappings.len() {
                0 => Err(PartMappingError::NoMappings),
                1 => Ok(matching_mappings),
                2.. => Err(PartMappingError::MultipleMatchingMappings(matching_mappings)),
            };

            let result = ProcessingResult { eda_placement, part_mappings: matching_mappings_result };
            results.push(result);
        }

        results
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub enum PartMappingError<'mapping> {
    MultipleMatchingMappings(Vec<&'mapping PartMapping<'mapping>>),
    NoMappings,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub struct ProcessingResult<'placement, 'mapping> {
    pub eda_placement: &'placement EdaPlacement,
    pub part_mappings: Result<Vec<&'mapping PartMapping<'mapping>>, PartMappingError<'mapping>>,
}

#[cfg(test)]
mod tests {
    use EdaPlacementDetails::DipTrace;
    use crate::pnp::part::Part;
    use crate::eda::diptrace::criteria::ExactMatchCriteria;
    use crate::eda::eda_placement::{DipTracePlacementDetails, EdaPlacement, EdaPlacementDetails};
    use crate::part_mapper::part_mapping::PartMapping;
    use crate::part_mapper::{PartMapper, PartMappingError, ProcessingResult};
    #[test]
    fn map_parts() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), details: DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE1".to_string() }) };
        let eda_placement2 = EdaPlacement { ref_des: "R2".to_string(), details: DipTrace(DipTracePlacementDetails { name: "NAME2".to_string(), value: "VALUE2".to_string() }) };
        let eda_placement3 = EdaPlacement { ref_des: "R3".to_string(), details: DipTrace(DipTracePlacementDetails { name: "NAME3".to_string(), value: "VALUE3".to_string() }) };

        let eda_placements = vec![eda_placement1, eda_placement2, eda_placement3];

        // and
        let part1 = Part::new("MFR1".to_string(), "PART1".to_string());
        let part2 = Part::new("MFR2".to_string(), "PART2".to_string());
        let part3 = Part::new("MFR3".to_string(), "PART3".to_string());

        let parts = vec![part1, part2, part3];

        // and
        let criteria1 = ExactMatchCriteria::new("NAME1".to_string(), "VALUE1".to_string());
        let part_mapping1 = PartMapping::new(&parts[1 - 1], vec![Box::new(criteria1)]);
        let criteria2 = ExactMatchCriteria::new("NAME2".to_string(), "VALUE2".to_string());
        let part_mapping2 = PartMapping::new(&parts[2 - 1], vec![Box::new(criteria2)]);
        let criteria3 = ExactMatchCriteria::new("NAME3".to_string(), "VALUE3".to_string());
        let part_mapping3 = PartMapping::new(&parts[3 - 1], vec![Box::new(criteria3)]);

        let part_mappings = vec![part_mapping1, part_mapping2, part_mapping3];

        // and
        let expected_results = vec![
            ProcessingResult { eda_placement: &eda_placements[0], part_mappings: Ok(vec![&part_mappings[0]]) },
            ProcessingResult { eda_placement: &eda_placements[1], part_mappings: Ok(vec![&part_mappings[1]]) },
            ProcessingResult { eda_placement: &eda_placements[2], part_mappings: Ok(vec![&part_mappings[2]]) },
        ];

        // when
        let matched_mappings = PartMapper::process(&eda_placements, &part_mappings);

        // then
        assert_eq!(matched_mappings, expected_results);
    }

    #[test]
    fn map_parts_with_multiple_matching_mappings() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), details: DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE1".to_string() }) };

        let eda_placements = vec![eda_placement1];

        // and
        let part1 = Part::new("MFR1".to_string(), "PART1".to_string());
        let part2 = Part::new("MFR2".to_string(), "PART2".to_string());

        let parts = vec![part1, part2];

        // and
        let criteria1 = ExactMatchCriteria::new("NAME1".to_string(), "VALUE1".to_string());
        let part_mapping1 = PartMapping::new(&parts[1 - 1], vec![Box::new(criteria1)]);
        let criteria2 = ExactMatchCriteria::new("NAME1".to_string(), "VALUE1".to_string());
        let part_mapping2 = PartMapping::new(&parts[2 - 1], vec![Box::new(criteria2)]);

        let part_mappings = vec![part_mapping1, part_mapping2];

        // and
        let expected_results = vec![
            ProcessingResult {
                eda_placement: &eda_placements[0],
                part_mappings: Err(PartMappingError::MultipleMatchingMappings(vec![&part_mappings[0], &part_mappings[1]]))
            },
        ];

        // when
        let matched_mappings = PartMapper::process(&eda_placements, &part_mappings);

        // then
        assert_eq!(matched_mappings, expected_results);
    }

    #[test]
    fn map_parts_with_no_part_mappings() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), details: DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE1".to_string() }) };

        let eda_placements = vec![eda_placement1];

        let part_mappings = vec![];

        // and
        let expected_results = vec![
            ProcessingResult {
                eda_placement: &eda_placements[0],
                part_mappings: Err(PartMappingError::NoMappings)
            },
        ];

        // when
        let matched_mappings = PartMapper::process(&eda_placements, &part_mappings);

        // then
        assert_eq!(matched_mappings, expected_results);
    }
}