use crate::eda::eda_placement::{EdaPlacement, EdaPlacementDetails};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaSubstitutionRuleCriteriaItem {
    pub field_name: String,
    // TODO replace with exact-match/regexp-match/etc, for now exact-match only.
    pub field_pattern: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaSubstitutionRuleChangeItem {
    pub field_name: String,
    pub field_value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaSubstitutionRule {
    pub criteria: Vec<EdaSubstitutionRuleCriteriaItem>,
    pub changes: Vec<EdaSubstitutionRuleChangeItem>,
}

impl EdaSubstitutionRule {
    pub fn format_criteria(&self) -> String {
        let mut result: Vec<String> = vec![];

        for criterion in self.criteria.iter() {
            result.push(format!("{}_pattern: '{}'", criterion.field_name, criterion.field_pattern));
        }
        return result.join(", ");
    }

    pub fn format_change(&self) -> String {
        let mut result: Vec<String> = vec![];

        for change in self.changes.iter() {
            result.push(format!("{}: '{}'", change.field_name, change.field_value));
        }
        return result.join(", ");
    }

    pub fn matches(&self, eda_placement: &EdaPlacement) -> bool {

        let result: Option<bool> = self.criteria.iter().fold(None, |mut matched, criterion| {
            // TODO update EdaPlacementDetails so it has a list of fields to make this trivial
            let field_matched = match (&eda_placement.details, criterion.field_name.as_str()) {
                (EdaPlacementDetails::DipTrace(details), "name") if criterion.field_pattern.eq(details.name.as_str()) => true,
                (EdaPlacementDetails::DipTrace(details), "value") if criterion.field_pattern.eq(details.value.as_str()) => true,
                (EdaPlacementDetails::KiCad(details), "package") if criterion.field_pattern.eq(details.package.as_str()) => true,
                (EdaPlacementDetails::KiCad(details), "val") if criterion.field_pattern.eq(details.val.as_str()) => true,
                _ => false,
            };

            if let Some(ref mut accumulated_result) = &mut matched {
                *accumulated_result &= field_matched;
            } else {
                matched = Some(field_matched)
            }

            matched
        });
        
        result.unwrap_or(false)
    }

    pub fn apply(&self, eda_placement: &EdaPlacement) -> EdaPlacement {
        let result = self.changes.iter().fold(eda_placement.clone(), |mut placement, change_item| {
            // TODO update EdaPlacement so it has a list of fields to make this trivial
            match (&mut placement.details, change_item.field_name.as_str()) {
                (EdaPlacementDetails::DipTrace(details), "name") => details.name = change_item.field_value.clone(),
                (EdaPlacementDetails::DipTrace(details), "value") => details.value = change_item.field_value.clone(),
                (EdaPlacementDetails::KiCad(details), "package") => details.package = change_item.field_value.clone(),
                (EdaPlacementDetails::KiCad(details), "val") => details.val = change_item.field_value.clone(),
                _ => (),
            }
            placement
        });

        result
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
    use crate::eda::eda_substitution::{EdaSubstitutionRule, EdaSubstitutionResult, EdaSubstitutor, EdaSubstitutionChainEntry, EdaSubstitutionRuleCriteriaItem, EdaSubstitutionRuleChangeItem};

    #[test]
    pub fn substitute_one_diptrace_placement_using_a_chain() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), place: true, details: EdaPlacementDetails::DipTrace(DipTracePlacementDetails { name: "NAME1".to_string(), value: "VALUE1".to_string() }) };
        let eda_placements= vec![eda_placement1];

        // and two substitution rules that must be applied

        let first_eda_substitution = EdaSubstitutionRule {
            criteria: vec![
                EdaSubstitutionRuleCriteriaItem { field_name: "name".to_string(), field_pattern: "NAME1".to_string() },
                EdaSubstitutionRuleCriteriaItem { field_name: "value".to_string(), field_pattern: "VALUE1".to_string() }
            ], changes: vec![
                EdaSubstitutionRuleChangeItem { field_name: "name".to_string(), field_value: "INTERMEDIATE_NAME1".to_string() },
                EdaSubstitutionRuleChangeItem { field_name: "value".to_string(), field_value: "INTERMEDIATE_VALUE1".to_string() }
            ],
        };
        let second_eda_substitution = EdaSubstitutionRule {
            criteria: vec![
                EdaSubstitutionRuleCriteriaItem { field_name: "name".to_string(), field_pattern: "INTERMEDIATE_NAME1".to_string() },
                EdaSubstitutionRuleCriteriaItem { field_name: "value".to_string(), field_pattern: "INTERMEDIATE_VALUE1".to_string() }
            ], changes: vec![
                EdaSubstitutionRuleChangeItem { field_name: "name".to_string(), field_value: "SUBSTITUTED_NAME1".to_string() },
                EdaSubstitutionRuleChangeItem { field_name: "value".to_string(), field_value: "SUBSTITUTED_VALUE1".to_string() }
            ],
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
            criteria: vec![
                EdaSubstitutionRuleCriteriaItem { field_name: "package".to_string(), field_pattern: "PACKAGE1".to_string() },
                EdaSubstitutionRuleCriteriaItem { field_name: "val".to_string(), field_pattern: "VAL1".to_string() }
            ], changes: vec![
                EdaSubstitutionRuleChangeItem { field_name: "package".to_string(), field_value: "INTERMEDIATE_PACKAGE1".to_string() },
                EdaSubstitutionRuleChangeItem { field_name: "val".to_string(), field_value: "INTERMEDIATE_VAL1".to_string() }
            ],
        };
        let second_eda_substitution = EdaSubstitutionRule {
            criteria: vec![
                EdaSubstitutionRuleCriteriaItem { field_name: "package".to_string(), field_pattern: "INTERMEDIATE_PACKAGE1".to_string() },
                EdaSubstitutionRuleCriteriaItem { field_name: "val".to_string(), field_pattern: "INTERMEDIATE_VAL1".to_string() }
            ], changes: vec![
                EdaSubstitutionRuleChangeItem { field_name: "package".to_string(), field_value: "SUBSTITUTED_PACKAGE1".to_string() },
                EdaSubstitutionRuleChangeItem { field_name: "val".to_string(), field_value: "SUBSTITUTED_VAL1".to_string() }
            ],
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
