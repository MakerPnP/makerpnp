use crate::eda::eda_placement::{EdaPlacement, EdaPlacementDetails};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaSubstitutionRule {
    pub details: EdaSubstitutionRuleDetails,
}

impl EdaSubstitutionRule {
    pub fn format_criteria(&self) -> String {
        // TODO consider using a trait for 'format_critera'
        match &self.details {
            EdaSubstitutionRuleDetails::DipTrace(details) => details.format_critera(),
            EdaSubstitutionRuleDetails::KiCad(details) => details.format_critera(),
        }
    }
}

impl EdaSubstitutionRule {
    pub fn format_change(&self) -> String {
        // TODO consider using a trait for 'format_change'
        match &self.details {
            EdaSubstitutionRuleDetails::DipTrace(details) => details.format_change(),
            EdaSubstitutionRuleDetails::KiCad(details) => details.format_change(),
        }
    }
}

impl EdaSubstitutionRule {
    pub fn matches(&self, eda_placement: &EdaPlacement) -> bool {
        match &self.details {
            EdaSubstitutionRuleDetails::DipTrace(details) if details.matches(eda_placement) => true,
            EdaSubstitutionRuleDetails::KiCad(details) if details.matches(eda_placement) => true,
            _ => false
        }
    }

    pub fn apply(&self, eda_placement: &EdaPlacement) -> EdaPlacement {
        // TODO error handling
        DetailsSubstitutor::substitute(&self.details, eda_placement).expect("OK")
    }

}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EdaSubstitutionRuleDetails {
    DipTrace(DipTraceSubstitutionRuleDetails),
    KiCad(KiCadSubstitutionRuleDetails),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DipTraceSubstitutionRuleDetails {
    // from
    pub name_pattern: String,
    pub value_pattern: String,

    // to
    pub name: String,
    pub value: String,
}

impl DipTraceSubstitutionRuleDetails {
    pub fn format_change(&self) -> String {
        format!("name: '{}', value: '{}'", self.name, self.value)
    }

    pub fn format_critera(&self) -> String {
        format!("name_pattern: '{}', value_pattern: '{}'", self.name_pattern, self.value_pattern)
    }

    pub fn matches(&self, eda_placement: &EdaPlacement) -> bool {
        match &eda_placement.details {
            EdaPlacementDetails::DipTrace(placement_details) => {
                self.name_pattern.eq(&placement_details.name) && self.value_pattern.eq(&placement_details.value)
            }
            _ => false
        }
    }
}


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KiCadSubstitutionRuleDetails {
    // from
    pub package_pattern: String,
    pub val_pattern: String,

    // to
    pub package: String,
    pub val: String,
}

impl KiCadSubstitutionRuleDetails {
    pub fn format_change(&self) -> String {
        format!("package: '{}', val: '{}'", self.package, self.val)
    }

    pub fn format_critera(&self) -> String {
        format!("package_pattern: '{}', val_pattern: '{}'", self.package_pattern, self.val_pattern)
    }

    pub fn matches(&self, eda_placement: &EdaPlacement) -> bool {
        match &eda_placement.details {
            EdaPlacementDetails::KiCad(placement_details) => {
                self.package_pattern.eq(&placement_details.package) && self.val_pattern.eq(&placement_details.val)
            }
            _ => false
        }
    }
}

struct DetailsSubstitutor {}

impl DetailsSubstitutor {
    pub fn substitute<'placement>(details: &EdaSubstitutionRuleDetails, eda_placement: &'placement EdaPlacement) -> Result<EdaPlacement, ()> {
        let mut substitute = eda_placement.clone();
        match (details, &mut substitute.details) {
            (EdaSubstitutionRuleDetails::DipTrace(ref source_details), EdaPlacementDetails::DipTrace(ref mut substitute_details)) => {
                substitute_details.name = source_details.name.clone();
                substitute_details.value = source_details.value.clone();

                Ok(substitute)
            }
            (EdaSubstitutionRuleDetails::KiCad(ref source_details), EdaPlacementDetails::KiCad(ref mut substitute_details)) => {
                substitute_details.package = source_details.package.clone();
                substitute_details.val = source_details.val.clone();

                Ok(substitute)
            },
            _ => {
                // TODO more descriptive error handling
                Err(())
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct EdaSubstitutionChainEntry<'rule> {
    pub rule: &'rule EdaSubstitutionRule,
}

#[derive(Debug, PartialEq)]
pub struct EdaSubstitutionResult<'placement, 'rule> {
    pub original_placement: &'placement EdaPlacement,
    pub resulting_placement: EdaPlacement,
    pub chain: Vec<EdaSubstitutionChainEntry<'rule>>,
}

pub struct EdaSubstitutor {}

impl EdaSubstitutor {
    pub fn substitute<'placement, 'rule>(original_eda_placements: &'placement [EdaPlacement], eda_substitution_rules: &'rule [EdaSubstitutionRule]) -> Vec<EdaSubstitutionResult<'placement, 'rule>> {

        let mut results = vec![];

        for original_eda_placement in original_eda_placements.iter() {

            let mut eda_placement = original_eda_placement.clone();
            let mut chain = vec![];

            loop {
                let mut applied_rule_count_this_pass = 0;

                for rule in eda_substitution_rules.iter() {
                    match rule.matches(&eda_placement) {
                        true => {
                            applied_rule_count_this_pass+= 1;
                            eda_placement = rule.apply(&eda_placement);

                            chain.push(EdaSubstitutionChainEntry { rule });
                        },
                        _ => ()
                    }
                }

                if applied_rule_count_this_pass == 0 {
                    break
                }
            }

            results.push( EdaSubstitutionResult {
                original_placement: original_eda_placement,
                resulting_placement: eda_placement,
                chain,
            })
        }

        results
    }
}

#[cfg(test)]
pub mod eda_substitutor_tests {
    use crate::eda::eda_placement::{DipTracePlacementDetails, EdaPlacement, EdaPlacementDetails, KiCadPlacementDetails};
    use crate::eda::eda_substitution::{DipTraceSubstitutionRuleDetails, EdaSubstitutionRule, EdaSubstitutionRuleDetails, EdaSubstitutionResult, EdaSubstitutor, EdaSubstitutionChainEntry, KiCadSubstitutionRuleDetails};

    #[test]
    pub fn substitute_one_diptrace_placement_using_a_chain() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), place: true, details: EdaPlacementDetails::DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE1".to_string() }) };
        let eda_placements= vec![eda_placement1];

        // and two substitution rules that must be applied
        let first_eda_substitution = EdaSubstitutionRule {
            details: EdaSubstitutionRuleDetails::DipTrace(DipTraceSubstitutionRuleDetails {
                name_pattern: "NAME1".to_string(),
                value_pattern: "VALUE1".to_string(),
                name: "INTERMEDIATE_NAME1".to_string(),
                value: "INTERMEDIATE_VALUE1".to_string(),
            })
        };
        let second_eda_substitution = EdaSubstitutionRule {
            details: EdaSubstitutionRuleDetails::DipTrace(DipTraceSubstitutionRuleDetails {
                name_pattern: "INTERMEDIATE_NAME1".to_string(),
                value_pattern: "INTERMEDIATE_VALUE1".to_string(),
                name: "SUBSTITUTED_NAME1".to_string(),
                value: "SUBSTITUTED_VALUE1".to_string(),
            })
        };
        // and a list of rules, that are out-of-order (i.e. eda_substitution1 must be applied first)
        let eda_substitutions= vec![second_eda_substitution, first_eda_substitution];

        // and
        let expected_results = vec![
            EdaSubstitutionResult {
                original_placement: &eda_placements[0],
                resulting_placement: EdaPlacement { ref_des: "R1".to_string(), place: true, details: EdaPlacementDetails::DipTrace(DipTracePlacementDetails { name: "SUBSTITUTED_NAME1".to_string(), value: "SUBSTITUTED_VALUE1".to_string() }) },
                chain: vec![
                    EdaSubstitutionChainEntry { rule: &eda_substitutions[1] },
                    EdaSubstitutionChainEntry { rule: &eda_substitutions[0] },
                ],
            }
        ];

        // when
        let results = EdaSubstitutor::substitute(
            eda_placements.as_slice(),
            eda_substitutions.as_slice()
        );

        // then
        assert_eq!(results, expected_results);
    }

    #[test]
    pub fn substitute_one_kicad_placement_using_a_chain() {
        // // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), place: true, details: EdaPlacementDetails::KiCad(KiCadPlacementDetails { package: "PACKAGE1".to_string(), val: "VAL1".to_string() }) };
        let eda_placements= vec![eda_placement1];

        // and two substitution rules that must be applied
        let first_eda_substitution = EdaSubstitutionRule {
            details: EdaSubstitutionRuleDetails::KiCad(KiCadSubstitutionRuleDetails {
                package_pattern: "PACKAGE1".to_string(),
                val_pattern: "VAL1".to_string(),
                package: "INTERMEDIATE_PACKAGE1".to_string(),
                val: "INTERMEDIATE_VAL1".to_string(),
            })
        };
        let second_eda_substitution = EdaSubstitutionRule {
            details: EdaSubstitutionRuleDetails::KiCad(KiCadSubstitutionRuleDetails {
                package_pattern: "INTERMEDIATE_PACKAGE1".to_string(),
                val_pattern: "INTERMEDIATE_VAL1".to_string(),
                package: "SUBSTITUTED_PACKAGE1".to_string(),
                val: "SUBSTITUTED_VAL1".to_string(),
            })
        };
        // and a list of rules, that are out-of-order (i.e. eda_substitution1 must be applied first)
        let eda_substitutions= vec![second_eda_substitution, first_eda_substitution];

        // and
        let expected_results = vec![
            EdaSubstitutionResult {
                original_placement: &eda_placements[0],
                resulting_placement: EdaPlacement { ref_des: "R1".to_string(), place: true, details: EdaPlacementDetails::KiCad(KiCadPlacementDetails { package: "SUBSTITUTED_PACKAGE1".to_string(), val: "SUBSTITUTED_VAL1".to_string() }) },
                chain: vec![
                    EdaSubstitutionChainEntry { rule: &eda_substitutions[1] },
                    EdaSubstitutionChainEntry { rule: &eda_substitutions[0] },
                ],
            }
        ];

        // when
        let results = EdaSubstitutor::substitute(
            eda_placements.as_slice(),
            eda_substitutions.as_slice()
        );

        // then
        assert_eq!(results, expected_results);
    }
}
