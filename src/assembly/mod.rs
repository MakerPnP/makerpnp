use thiserror::Error;

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct ProcessingResult {
    pub placements: Vec<Placement>,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
#[derive(Clone)]
pub struct Placement {
    pub ref_des: String,
}

pub struct AssemblyVariant {
    pub name: String,
    pub ref_des_list: Vec<String>,
}

impl AssemblyVariant {
    pub fn new(name: String, variant_refdes_list: Vec<String>) -> Self {
        Self {
            name,
            ref_des_list: variant_refdes_list
        }
    }
}

impl Placement {
    pub fn new(ref_des: String) -> Self {
        Self {
            ref_des
        }
    }
}

impl ProcessingResult {
    pub fn new(placements: Vec<Placement>) -> Self {
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
    pub fn process(&self, placements: Vec<Placement>, variant: AssemblyVariant) -> Result<ProcessingResult, ProcessingError> {
        if placements.is_empty() {
            return Err(ProcessingError::NoPlacements)
        }
        if variant.ref_des_list.is_empty() {
            return Err(ProcessingError::EmptyRefDesList)
        }


        let variant_placements: Vec<Placement> = placements.iter().cloned().filter(|placement| {
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
    use crate::assembly::{AssemblyVariantProcessor, Placement, ProcessingResult, AssemblyVariant, ProcessingError};

    #[test]
    fn process() {

        // given
        let placement1 = Placement::new(String::from("R1"));
        let placement2 = Placement::new(String::from("R2"));
        let placement3 = Placement::new(String::from("R3"));
        let placement4 = Placement::new(String::from("D1"));
        let placement5 = Placement::new(String::from("D2"));
        let placement6 = Placement::new(String::from("D3"));
        let placement7 = Placement::new(String::from("C1"));
        let placement8 = Placement::new(String::from("J1"));

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
        let result = assembly_variant_processor.process(all_placements, variant);

        // then
        assert_eq!(result, expected_result);
    }

    #[test]
    fn no_placements() {
        // given
        let variant = AssemblyVariant::new(String::from("Variant 1"), vec![]);
        let assembly_variant_processor = AssemblyVariantProcessor::default();

        // when
        let result = assembly_variant_processor.process(vec![], variant);

        // then
        assert_eq!(result, Err(ProcessingError::NoPlacements));
    }

    #[test]
    fn empty_variant_refdes_list() {
        // given
        let variant = AssemblyVariant::new(String::from("Variant 1"), vec![]);
        let assembly_variant_processor = AssemblyVariantProcessor::default();

        let placement1 = Placement::new(String::from("R1"));

        // when
        let result = assembly_variant_processor.process(vec![placement1], variant);

        // then
        assert_eq!(result, Err(ProcessingError::EmptyRefDesList));
    }
}