use crate::eda::eda_placement::{EdaPlacement, EdaPlacementDetails};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaSubstitutionRule {
    pub details: EdaSubstitutionRuleDetails,
}

impl EdaSubstitutionRule {
    pub fn format_criteria(&self) -> String {
        match &self.details {
            EdaSubstitutionRuleDetails::DipTrace(details) => details.format_critera(),
        }
    }
}

impl EdaSubstitutionRule {
    pub fn format_change(&self) -> String {
        match &self.details {
            EdaSubstitutionRuleDetails::DipTrace(details) => details.format_change(),
        }
    }
}

impl EdaSubstitutionRule {
    pub fn matches(&self, eda_placement: &EdaPlacement) -> bool {
        match &self.details {
            EdaSubstitutionRuleDetails::DipTrace(details) if details.matches(eda_placement) => true,
            _ => false
        }
    }

    pub fn apply(&self, eda_placement: &EdaPlacement) -> EdaPlacement {
        match &self.details {
            EdaSubstitutionRuleDetails::DipTrace(details) => details.substitute(eda_placement),
        }
    }

}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EdaSubstitutionRuleDetails {
    DipTrace(DipTraceSubstitutionRuleDetails)
    //...KiCad(KiCadSubstitutionDetails)
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
        }
    }

    pub fn substitute<'placement>(&self, eda_placement: &'placement EdaPlacement) -> EdaPlacement {
        let mut substitute = eda_placement.clone();
        match substitute.details {
            EdaPlacementDetails::DipTrace(ref mut placement_details) => {
                placement_details.name = self.name.clone();
                placement_details.value = self.value.clone();
            }
        }

        substitute
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
    use crate::eda::eda_placement::{DipTracePlacementDetails, EdaPlacement, EdaPlacementDetails};
    use crate::eda::eda_substitution::{DipTraceSubstitutionRuleDetails, EdaSubstitutionRule, EdaSubstitutionRuleDetails, EdaSubstitutionResult, EdaSubstitutor, EdaSubstitutionChainEntry};

    #[test]
    pub fn substitute_one_placement_using_a_chain() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), details: EdaPlacementDetails::DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE1".to_string() }) };
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
                resulting_placement: EdaPlacement { ref_des: "R1".to_string(), details: EdaPlacementDetails::DipTrace(DipTracePlacementDetails { name: "SUBSTITUTED_NAME1".to_string(), value: "SUBSTITUTED_VALUE1".to_string() }) },
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
