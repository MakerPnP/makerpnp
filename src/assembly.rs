#[cfg_attr(test, derive(PartialEq, Debug))]
struct ProcessingResult<'placements> {
    placements: Vec<&'placements Placement>,
}

#[cfg_attr(test, derive(PartialEq, Debug))]
struct Placement {
    refdes: String,
}

struct Variant {
    name: String,
    variant_refdes_list: Vec<String>,
}

impl Variant {
    pub fn new(name: String, variant_refdes_list: Vec<String>) -> Self {
        Self {
            name,
            variant_refdes_list
        }
    }
}

impl Placement {
    pub fn new(refdes: String) -> Self {
        Self {
            refdes
        }
    }
}

impl<'placements> ProcessingResult<'placements> {
    pub fn new(placements: Vec<&'placements Placement>) -> Self {
        Self {
            placements
        }
    }
}

struct AssemblyVariantProcessor {}

impl AssemblyVariantProcessor {
    pub fn process<'placements>(&self, placements: Vec<&'placements Placement>, variant: Variant) -> Result<ProcessingResult<'placements>, ()> {
        if placements.is_empty() || variant.variant_refdes_list.is_empty() {
            return Err(())
        }


        let variant_placements: Vec<&Placement> = placements.iter().cloned().filter(|placement| {
            variant.variant_refdes_list.contains(&placement.refdes)

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
    use crate::assembly::{AssemblyVariantProcessor, Placement, ProcessingResult, Variant};

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
            &placement1, &placement2, &placement3,
            &placement4, &placement5, &placement6,
            &placement7, &placement8
        ];

        // and
        let variant_refdes_list = vec![
            String::from("R1"),
            String::from("D1"),
            String::from("C1"),
            String::from("J1"),
        ];

        let variant = Variant::new(
            String::from("Variant 1"),
            variant_refdes_list,
        );

        // and
        let variant_placements = vec![&placement1, &placement4, &placement7, &placement8];
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
        let variant = Variant::new(String::from("Variant 1"), vec![]);
        let assembly_variant_processor = AssemblyVariantProcessor::default();

        // when
        let result = assembly_variant_processor.process(vec![], variant);

        // then
        assert_eq!(result, Err(()));
    }

    #[test]
    fn empty_variant_refdes_list() {
        // given
        let variant = Variant::new(String::from("Variant 1"), vec![]);
        let assembly_variant_processor = AssemblyVariantProcessor::default();

        let placement1 = Placement::new(String::from("R1"));

        // when
        let result = assembly_variant_processor.process(vec![&placement1], variant);

        // then
        assert_eq!(result, Err(()));
    }
}