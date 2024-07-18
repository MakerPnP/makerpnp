use thiserror::Error;
use crate::eda::assembly_variant::AssemblyVariant;
use crate::eda::eda_placement::EdaPlacement;

pub mod rules;

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
    #[error("Empty ref-des list")]
    EmptyRefDesList,
}

pub struct AssemblyVariantProcessor {}

impl AssemblyVariantProcessor {
    pub fn process(&self, placements: &Vec<EdaPlacement>, variant: AssemblyVariant) -> Result<ProcessingResult, ProcessingError> {
        if placements.is_empty() {
            return Err(ProcessingError::NoPlacements)
        }
        if variant.ref_des_list.is_empty() {
            return Err(ProcessingError::EmptyRefDesList)
        }

        let variant_placements: Vec<EdaPlacement> = placements.iter().cloned().filter(|placement| {
            variant.ref_des_list.contains(&placement.ref_des)
        }).collect();

        Ok(ProcessingResult::new(variant_placements))
    }
}

impl Default for AssemblyVariantProcessor {
    fn default() -> Self {
        Self {}
    }
}

#[cfg(test)]
mod test {
    use crate::assembly::{AssemblyVariantProcessor, ProcessingError, ProcessingResult};
    use crate::eda::assembly_variant::AssemblyVariant;
    use crate::eda::eda_placement::{DipTracePlacementDetails, EdaPlacement};
    use crate::eda::eda_placement::EdaPlacementDetails::DipTrace;

    #[test]
    fn process() {
        // given
        let placement1 = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE1".to_string() }),
        };
        let placement2 = EdaPlacement {
            ref_des: "R2".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME2".to_string(), value: "VALUE2".to_string() }),
        };
        let placement3 = EdaPlacement {
            ref_des: "R3".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME3".to_string(), value: "VALUE3".to_string() }),
        };
        let placement4 = EdaPlacement {
            ref_des: "D1".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME4".to_string(), value: "VALUE4".to_string() }),
        };
        let placement5 = EdaPlacement {
            ref_des: "D2".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME5".to_string(), value: "VALUE5".to_string() }),
        };
        let placement6 = EdaPlacement {
            ref_des: "D3".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME6".to_string(), value: "VALUE6".to_string() }),
        };
        let placement7 = EdaPlacement {
            ref_des: "C1".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME7".to_string(), value: "VALUE7".to_string() }),
        };
        let placement8 = EdaPlacement {
            ref_des: "J1".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME8".to_string(), value: "VALUE8".to_string() }),
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
        let expected_result = Result::Ok(ProcessingResult::new(variant_placements));

        // and
        let assembly_variant_processor = AssemblyVariantProcessor::default();

        // when
        let result = assembly_variant_processor.process(&all_placements, variant);

        // then
        assert_eq!(result, expected_result);
    }

    #[test]
    fn no_placements() {
        // given
        let variant = AssemblyVariant::new(String::from("Variant 1"), vec![]);
        let assembly_variant_processor = AssemblyVariantProcessor::default();

        // when
        let result = assembly_variant_processor.process(&vec![], variant);

        // then
        assert_eq!(result, Err(ProcessingError::NoPlacements));
    }

    #[test]
    fn empty_variant_refdes_list() {
        // given
        let variant = AssemblyVariant::new(String::from("Variant 1"), vec![]);
        let assembly_variant_processor = AssemblyVariantProcessor::default();

        let placement1 = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            details: DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE1".to_string() }),
        };
        // when
        let result = assembly_variant_processor.process(&vec![placement1], variant);

        // then
        assert_eq!(result, Err(ProcessingError::EmptyRefDesList));
    }
}