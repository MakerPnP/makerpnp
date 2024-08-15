use crate::eda::placement::EdaPlacement;
use crate::eda::criteria::FieldCriterion;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdaSubstitutionRuleTransformItem {
    pub field_name: String,
    pub field_value: String,
}

#[derive(Debug, PartialEq)]
pub struct EdaSubstitutionRule {
    pub criteria: Vec<Box<dyn FieldCriterion>>,
    pub transforms: Vec<EdaSubstitutionRuleTransformItem>,
}

impl EdaSubstitutionRule {
    pub fn format_criteria(&self) -> String {
        let mut result: Vec<String> = vec![];

        for criterion in self.criteria.iter() {
            result.push(format!("{criterion:}"));
        }
        result.join(", ")
    }

    pub fn format_transform(&self) -> String {
        let mut result: Vec<String> = vec![];

        for transform in self.transforms.iter() {
            result.push(format!("{}: '{}'", transform.field_name, transform.field_value));
        }
        result.join(", ")
    }

    pub fn matches(&self, eda_placement: &EdaPlacement) -> bool {

        let result: Option<bool> = self.criteria.iter().fold(None, |mut matched, criterion| {
            let matched_field = eda_placement.fields.iter().find(|field | {
                (*criterion).matches(field.name.as_str(), field.value.as_str())
            });

            match (&mut matched, matched_field) {
                // matched, previous fields checked
                (Some(accumulated_result), Some(_field)) => *accumulated_result &= true,
                // matched, first field
                (None, Some(_field)) => matched = Some(true),
                // not matched, previous fields checked
                (Some(accumulated_result), None) => *accumulated_result = false,
                // not matched, first field
                (None, None) => matched = Some(false),
            }

            matched
        });

        result.unwrap_or(false)
    }

    pub fn apply(&self, eda_placement: &EdaPlacement) -> EdaPlacement {
        let result = self.transforms.iter().fold(eda_placement.clone(), |mut placement, change_item| {
            if let Some(field) = placement.fields.iter_mut().find(|field| field.name.eq(change_item.field_name.as_str())) {
                field.value.clone_from(&change_item.field_value);
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

            // FIXME if the rule makes no modification to the placement, then it will match again on the next iteration causing an infinite loop
            //       consider filtering out invalid rules at the source instead of handling this situation here (where it is too late to do anything about)
            loop {
                let mut applied_rule_count_this_pass = 0;

                for rule in eda_substitution_rules.iter() {
                    if rule.matches(&eda_placement) {
                        applied_rule_count_this_pass+= 1;
                        eda_placement = rule.apply(&eda_placement);

                        chain.push(EdaSubstitutionChainEntry { rule });
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
    use regex::Regex;
    use crate::eda::placement::{EdaPlacement, EdaPlacementField };
    use crate::eda::substitution::{EdaSubstitutionRule, EdaSubstitutionResult, EdaSubstitutor, EdaSubstitutionChainEntry, EdaSubstitutionRuleTransformItem};
    use crate::eda::criteria::{ExactMatchCriterion, RegexMatchCriterion};
    
    #[test]
    pub fn substitute_one_diptrace_placement_using_a_chain() {
        // given
        let eda_placement1 = EdaPlacement {
            ref_des: "R1".to_string(),
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME1".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE1".to_string()),
            ],
            ..EdaPlacement::default()
        };
        let eda_placements= vec![eda_placement1];

        // and two substitution rules that must be applied

        let first_eda_substitution_rule = EdaSubstitutionRule {
            criteria: vec![
                Box::new(ExactMatchCriterion { field_name: "name".to_string(), field_pattern: "NAME1".to_string() }),
                Box::new(RegexMatchCriterion { field_name: "value".to_string(), field_pattern: Regex::new("VALUE1").unwrap() }),
            ],
            transforms: vec![
                EdaSubstitutionRuleTransformItem { field_name: "name".to_string(), field_value: "INTERMEDIATE_NAME1".to_string() },
                EdaSubstitutionRuleTransformItem { field_name: "value".to_string(), field_value: "INTERMEDIATE_VALUE1".to_string() }
            ],
        };
        let second_eda_substitution_rule = EdaSubstitutionRule {
            criteria: vec![
                Box::new(ExactMatchCriterion { field_name: "name".to_string(), field_pattern: "INTERMEDIATE_NAME1".to_string() }),
                Box::new(ExactMatchCriterion { field_name: "value".to_string(), field_pattern: "INTERMEDIATE_VALUE1".to_string() }),
            ],
            transforms: vec![
                EdaSubstitutionRuleTransformItem { field_name: "name".to_string(), field_value: "SUBSTITUTED_NAME1".to_string() },
                EdaSubstitutionRuleTransformItem { field_name: "value".to_string(), field_value: "SUBSTITUTED_VALUE1".to_string() }
            ],
        };
        // and a list of rules, that are out-of-order (i.e. eda_substitution1 must be applied first)
        let eda_substitution_rules = vec![second_eda_substitution_rule, first_eda_substitution_rule];
        println!("{:?}", eda_substitution_rules);

        // and
        let expected_results = vec![
            EdaSubstitutionResult {
                original_placement: &eda_placements[0],
                resulting_placement: EdaPlacement {
                    ref_des: "R1".to_string(),
                    fields: vec![
                        EdaPlacementField::new("name".to_string(), "SUBSTITUTED_NAME1".to_string()),
                        EdaPlacementField::new("value".to_string(), "SUBSTITUTED_VALUE1".to_string()),
                    ],
                    ..EdaPlacement::default()
                },
                chain: vec![
                    EdaSubstitutionChainEntry { rule: &eda_substitution_rules[1] },
                    EdaSubstitutionChainEntry { rule: &eda_substitution_rules[0] },
                ],
            }
        ];

        // when
        let results = EdaSubstitutor::substitute(
            eda_placements.as_slice(),
            eda_substitution_rules.as_slice()
        );

        // then
        assert_eq!(results, expected_results);
    }

    #[test]
    pub fn substitute_one_kicad_placement_using_a_chain() {
        // // given
        let eda_placement1 = EdaPlacement { 
            ref_des: "R1".to_string(), 
            fields: vec![
                EdaPlacementField::new("package".to_string(), "PACKAGE1".to_string()),
                EdaPlacementField::new("val".to_string(), "VAL1".to_string()),
            ],
            ..EdaPlacement::default()
        };
        let eda_placements= vec![eda_placement1];

        // and two substitution rules that must be applied
        let first_eda_substitution = EdaSubstitutionRule {
            criteria: vec![
                Box::new(ExactMatchCriterion { field_name: "package".to_string(), field_pattern: "PACKAGE1".to_string() }),
                Box::new(ExactMatchCriterion { field_name: "val".to_string(), field_pattern: "VAL1".to_string() }),
            ],
            transforms: vec![
                EdaSubstitutionRuleTransformItem { field_name: "package".to_string(), field_value: "INTERMEDIATE_PACKAGE1".to_string() },
                EdaSubstitutionRuleTransformItem { field_name: "val".to_string(), field_value: "INTERMEDIATE_VAL1".to_string() }
            ],
        };
        let second_eda_substitution = EdaSubstitutionRule {
            criteria: vec![
                Box::new(ExactMatchCriterion { field_name: "package".to_string(), field_pattern: "INTERMEDIATE_PACKAGE1".to_string() }),
                Box::new(ExactMatchCriterion { field_name: "val".to_string(), field_pattern: "INTERMEDIATE_VAL1".to_string() }),
            ],
            transforms: vec![
                EdaSubstitutionRuleTransformItem { field_name: "package".to_string(), field_value: "SUBSTITUTED_PACKAGE1".to_string() },
                EdaSubstitutionRuleTransformItem { field_name: "val".to_string(), field_value: "SUBSTITUTED_VAL1".to_string() }
            ],
        };
        // and a list of rules, that are out-of-order (i.e. eda_substitution1 must be applied first)
        let eda_substitutions= vec![second_eda_substitution, first_eda_substitution];

        // and
        let expected_results = vec![
            EdaSubstitutionResult {
                original_placement: &eda_placements[0],
                resulting_placement: EdaPlacement {
                    ref_des: "R1".to_string(),
                    fields: vec![
                        EdaPlacementField::new("package".to_string(), "SUBSTITUTED_PACKAGE1".to_string()),
                        EdaPlacementField::new("val".to_string(), "SUBSTITUTED_VAL1".to_string()),
                    ],
                    ..EdaPlacement::default()
                },
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
