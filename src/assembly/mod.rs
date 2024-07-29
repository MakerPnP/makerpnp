use thiserror::Error;
use assembly_variant::AssemblyVariant;
use crate::eda::placement::EdaPlacement;

pub mod rules;
pub mod assembly_variant;

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct ProcessingResult {
    pub placements: Vec<EdaPlacement>,
}

impl ProcessingResult {
    pub fn new(placements: Vec<EdaPlacement>) -> Self {
        Self {
            placements
        }
    }
}

#[derive(Error, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ProcessingError {
    #[error("No placements")]
    NoPlacements,
}

pub struct AssemblyVariantProcessor {}

impl AssemblyVariantProcessor {
    pub fn process(placements: &[EdaPlacement], variant: AssemblyVariant) -> Result<ProcessingResult, ProcessingError> {
        if placements.is_empty() {
            return Err(ProcessingError::NoPlacements)
        }

        let variant_placements: Vec<EdaPlacement> = placements.iter().filter_map(|placement| {
            if variant.ref_des_list.is_empty() || variant.ref_des_list.contains(&placement.ref_des) {
                Some(placement.clone())
            } else {
                None
            }
        }).collect();

        Ok(ProcessingResult::new(variant_placements))
    }
}

#[cfg(test)]
mod test {
    use crate::assembly::{AssemblyVariantProcessor, ProcessingError, ProcessingResult};
    use crate::assembly::assembly_variant::AssemblyVariant;
    use crate::eda::placement::{EdaPlacement, EdaPlacementField};

    #[test]
    fn process() {
        // given
        let placement1 = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME1".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE1".to_string()),
            ],
        };
        let placement2 = EdaPlacement {
            ref_des: "R2".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME2".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE2".to_string()),
            ],
        };
        let placement3 = EdaPlacement {
            ref_des: "R3".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME3".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE3".to_string()),
            ],        };
        let placement4 = EdaPlacement {
            ref_des: "D1".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME4".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE4".to_string()),
            ],        };
        let placement5 = EdaPlacement {
            ref_des: "D2".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME5".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE5".to_string()),
            ],        };
        let placement6 = EdaPlacement {
            ref_des: "D3".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME6".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE6".to_string()),
            ],        };
        let placement7 = EdaPlacement {
            ref_des: "C1".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME7".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE7".to_string()),
            ],        };
        let placement8 = EdaPlacement {
            ref_des: "J1".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME8".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE8".to_string()),
            ],
        };

        let all_placements = vec![
            placement1, placement2, placement3,
            placement4, placement5, placement6,
            placement7, placement8
        ];

        // and
        let variant_refdes_list = vec![
            String::from("R1"),
            String::from("D1"),
            String::from("C1"),
            String::from("J1"),
        ];

        let variant = AssemblyVariant::new(
            String::from("Variant 1"),
            variant_refdes_list,
        );

        // and
        let variant_placements = vec![
            all_placements[1-1].clone(),
            all_placements[4-1].clone(),
            all_placements[7-1].clone(),
            all_placements[8-1].clone(),
        ];
        let expected_result = Ok(ProcessingResult::new(variant_placements));
        
        // when
        let result = AssemblyVariantProcessor::process(&all_placements, variant);

        // then
        assert_eq!(result, expected_result);
    }

    #[test]
    fn no_placements() {
        // given
        let variant = AssemblyVariant::new(String::from("Variant 1"), vec![]);

        // when
        let result = AssemblyVariantProcessor::process(&[], variant);

        // then
        assert_eq!(result, Err(ProcessingError::NoPlacements));
    }

    #[test]
    fn empty_variant_refdes_list() {
        // given
        let variant = AssemblyVariant::new(String::from("Variant 1"), vec![]);

        let placement1 = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME1".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE1".to_string()),
            ],
        };

        let all_placements = vec![
            placement1
        ];

        let expected_variant_placements = vec![
            all_placements[1-1].clone(),
        ];

        // and
        let expected_result = Ok(ProcessingResult::new(expected_variant_placements));

        // when
        let result = AssemblyVariantProcessor::process(&all_placements, variant);

        // then
        assert_eq!(result, expected_result);
    }
}