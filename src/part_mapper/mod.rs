pub mod criteria;
pub mod part_mapping;

use std::fmt::{Display, Formatter};
use crate::assembly::rules::AssemblyRule;
use crate::eda::placement::EdaPlacement;
use crate::part_mapper::part_mapping::PartMapping;
use crate::part_mapper::PartMappingError::{ConflictingRules, NoRulesApplied};
use crate::pnp::load_out_item::LoadOutItem;
use crate::pnp::part::Part;

pub struct PartMapper {}

impl PartMapper {
    pub fn process<'placement, 'mapping>(
        eda_placements: &'placement [EdaPlacement],
        part_mappings: &'mapping [PartMapping<'mapping>],
        load_out_items: &[LoadOutItem],
        assembly_rules: &[AssemblyRule]
    ) -> Result<Vec<PlacementPartMappingResult<'placement, 'mapping>>, PartMapperError<'placement, 'mapping>> {

        let mut error_count: usize = 0;
        let mut mappings = vec![];

        for eda_placement in eda_placements.iter() {
            let mut part_mapping_results = vec![];

            for part_mapping in part_mappings.iter() {
                for criteria in part_mapping.criteria.iter() {
                    if criteria.matches(eda_placement) {
                        part_mapping_results.push(PartMappingResult { part_mapping, applied_rule: None });
                    }
                }
            }

            apply_rules(&eda_placement.ref_des, &mut part_mapping_results, load_out_items, assembly_rules);

            let applied_rule_count = part_mapping_results.iter().filter(|pmr|pmr.applied_rule.is_some()).count();

            let (mapping_result, part) = match (part_mapping_results.len(), applied_rule_count) {
                (_, 1) => {
                    let part = part_mapping_results.iter().find(|it|it.applied_rule.is_some()).unwrap().part_mapping.part;
                    (Ok(part_mapping_results), Some(part))
                },
                (0, _) => (Err(PartMappingError::NoMappings), None),
                (1.., 0) => (Err(NoRulesApplied(part_mapping_results)), None),
                (_, 2..) => (Err(ConflictingRules(part_mapping_results)), None),
            };

            if mapping_result.is_err() {
                error_count += 1
            }

            let result = PlacementPartMappingResult { part, eda_placement, mapping_result };
            mappings.push(result);
        }

        match error_count {
            0 => Ok(mappings),
            1.. => Err(PartMapperError::MappingErrors(mappings))
        }
    }
}

fn apply_rules<'mapping>(ref_des: &String, mapping_results: &mut [PartMappingResult<'mapping>], load_out_items: &[LoadOutItem], assembly_rules: &[AssemblyRule]) {
    for mapping_result in mapping_results.iter_mut() {
        let maybe_assembly_rule = assembly_rules.iter().find(|rule| {
            let mapped_part = mapping_result.part_mapping;
            *ref_des == rule.ref_des &&
                mapped_part.part.manufacturer == rule.manufacturer &&
                mapped_part.part.mpn == rule.mpn
        });

        if let Some(_rule) = maybe_assembly_rule {
            mapping_result.applied_rule = Some(AppliedMappingRule::AssemblyRule);
            return
        }
    }

    match mapping_results.len() {
        1 => {
            mapping_results[0].applied_rule = Some(AppliedMappingRule::AutoSelected);
        }
        2.. => {
            for mapping_result in mapping_results.iter_mut() {
                let maybe_load_out_item = load_out_items.iter().find(|item| {
                    let mapped_part = mapping_result.part_mapping;
                    (item.mpn == mapped_part.part.mpn)
                        && (item.manufacturer == mapped_part.part.manufacturer)
                });

                if let Some(load_out_item) = maybe_load_out_item {
                    mapping_result.applied_rule = Some(AppliedMappingRule::FoundInLoadOut(load_out_item.reference.clone()));
                }
            }
        },
        _ => (),
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub enum PartMapperError<'placement, 'mapping> {
    MappingErrors(Vec<PlacementPartMappingResult<'placement, 'mapping>>),
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub enum PartMappingError<'mapping> {
    /// Multiple rules were applied, when there should be only one.
    ConflictingRules(Vec<PartMappingResult<'mapping>>),
    /// Mappings exist, but no rules were applied.
    NoRulesApplied(Vec<PartMappingResult<'mapping>>),
    /// No mappings to apply rules to.
    NoMappings,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub enum AppliedMappingRule {
    AutoSelected,
    FoundInLoadOut(String),
    AssemblyRule,
}

impl Display for AppliedMappingRule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppliedMappingRule::AutoSelected => write!(f, "Auto-selected"),
            AppliedMappingRule::FoundInLoadOut(reference) => write!(f, "Found in load-out, reference: '{}'", reference),
            AppliedMappingRule::AssemblyRule => write!(f, "Matched assembly-rule"),
        }
    }
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub struct PartMappingResult<'mapping> {
    pub part_mapping: &'mapping PartMapping<'mapping>,
    pub applied_rule: Option<AppliedMappingRule>,
}

#[cfg_attr(test, derive(PartialEq))]
#[derive(Debug)]
pub struct PlacementPartMappingResult<'placement, 'mapping> {
    pub eda_placement: &'placement EdaPlacement,
    pub mapping_result: Result<Vec<PartMappingResult<'mapping>>, PartMappingError<'mapping>>,
    pub part: Option<&'mapping Part>
}

#[cfg(test)]
mod tests {
    use crate::assembly::rules::AssemblyRule;
    use crate::eda::criteria::{GenericCriteriaItem, GenericExactMatchCriteria};
    use crate::pnp::part::Part;
    use crate::eda::placement::{EdaPlacement, EdaPlacementField};
    use crate::part_mapper::part_mapping::PartMapping;
    use crate::part_mapper::{AppliedMappingRule, PartMapper, PartMapperError, PartMappingError, PartMappingResult, PlacementPartMappingResult};
    use crate::pnp::load_out_item::LoadOutItem;

    #[test]
    fn map_parts() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME1".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE1".to_string()),
            ],
        };
        let eda_placement2 = EdaPlacement { ref_des: "R2".to_string(), place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME2".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE2".to_string()),
            ],
        };
        let eda_placement3 = EdaPlacement { ref_des: "R3".to_string(), place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME3".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE3".to_string()),
            ],
        };

        let eda_placements = vec![eda_placement1, eda_placement2, eda_placement3];

        // and
        let part1 = Part::new("MFR1".to_string(), "PART1".to_string());
        let part2 = Part::new("MFR2".to_string(), "PART2".to_string());
        let part3 = Part::new("MFR3".to_string(), "PART3".to_string());

        let parts = [part1, part2, part3];

        // and
        let criteria1 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME1".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE1".to_string() ),
        ]};
        let part_mapping1 = PartMapping::new(&parts[1 - 1], vec![Box::new(criteria1)]);
        let criteria2 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME2".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE2".to_string() ),
        ]};
        let part_mapping2 = PartMapping::new(&parts[2 - 1], vec![Box::new(criteria2)]);
        let criteria3 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME3".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE3".to_string() ),
        ]};
        let part_mapping3 = PartMapping::new(&parts[3 - 1], vec![Box::new(criteria3)]);

        let part_mappings = vec![part_mapping1, part_mapping2, part_mapping3];

        // and
        let expected_results = Ok(vec![
            PlacementPartMappingResult { part: Some(&parts[0]), eda_placement: &eda_placements[0], mapping_result: Ok(vec![PartMappingResult { part_mapping: &part_mappings[0], applied_rule: Some(AppliedMappingRule::AutoSelected) }]) },
            PlacementPartMappingResult { part: Some(&parts[1]), eda_placement: &eda_placements[1], mapping_result: Ok(vec![PartMappingResult { part_mapping: &part_mappings[1], applied_rule: Some(AppliedMappingRule::AutoSelected) }]) },
            PlacementPartMappingResult { part: Some(&parts[2]), eda_placement: &eda_placements[2], mapping_result: Ok(vec![PartMappingResult { part_mapping: &part_mappings[2], applied_rule: Some(AppliedMappingRule::AutoSelected) }]) },
        ]);

        // when
        let matched_mappings = PartMapper::process(&eda_placements, &part_mappings, &[], &[]);

        // then
        assert_eq!(matched_mappings, expected_results);
    }

    #[test]
    fn map_parts_with_multiple_matching_mappings() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME1".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE1".to_string()),
            ],
        };

        let eda_placements = vec![eda_placement1];

        // and
        let part1 = Part::new("MFR1".to_string(), "PART1".to_string());
        let part2 = Part::new("MFR2".to_string(), "PART2".to_string());

        let parts = [part1, part2];

        // and
        let criteria1 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME1".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE1".to_string() ),
        ]};
        let part_mapping1 = PartMapping::new(&parts[1 - 1], vec![Box::new(criteria1)]);
        let criteria2 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME1".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE1".to_string() ),
        ]};
        let part_mapping2 = PartMapping::new(&parts[2 - 1], vec![Box::new(criteria2)]);

        let part_mappings = vec![part_mapping1, part_mapping2];

        // and
        let expected_results = Err(PartMapperError::MappingErrors(vec![
            PlacementPartMappingResult {
                part: None,
                eda_placement: &eda_placements[0],
                mapping_result: Err(PartMappingError::NoRulesApplied(vec![
                    PartMappingResult { part_mapping: &part_mappings[0], applied_rule: None },
                    PartMappingResult { part_mapping: &part_mappings[1], applied_rule: None },
                ]))
            },
        ]));

        // when
        let matched_mappings = PartMapper::process(&eda_placements, &part_mappings, &[], &[]);

        // then
        assert_eq!(matched_mappings, expected_results);
    }

    #[test]
    fn map_parts_with_no_part_mappings() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME1".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE1".to_string()),
            ],
        };

        let eda_placements = vec![eda_placement1];

        let part_mappings = vec![];

        // and
        let expected_results = Err(PartMapperError::MappingErrors(vec![
            PlacementPartMappingResult {
                part: None,
                eda_placement: &eda_placements[0],
                mapping_result: Err(PartMappingError::NoMappings)
            },
        ]));

        // when
        let matched_mappings = PartMapper::process(&eda_placements, &part_mappings, &[], &[]);

        // then
        assert_eq!(matched_mappings, expected_results);
    }

    #[test]
    fn map_parts_with_multiple_matching_mappings_with_one_in_the_load_out() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME1".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE1".to_string()),
            ],
        };

        let eda_placements = vec![eda_placement1];

        // and
        let part1 = Part::new("MFR1".to_string(), "PART1".to_string());
        let part2 = Part::new("MFR2".to_string(), "PART2".to_string());
        let part3 = Part::new("MFR3".to_string(), "PART3".to_string());

        let parts = [part1, part2, part3];

        // and
        let criteria1 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME1".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE1".to_string() ),
        ]};
        let part_mapping1 = PartMapping::new(&parts[1 - 1], vec![Box::new(criteria1)]);
        let criteria2 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME1".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE1".to_string() ),
        ]};
        let part_mapping2 = PartMapping::new(&parts[2 - 1], vec![Box::new(criteria2)]);

        let part_mappings = vec![part_mapping1, part_mapping2];

        // and
        let load_out_items = vec![
            LoadOutItem::new("REFERENCE_1".to_string(), "MFR3".to_string(), "PART3".to_string()),
            LoadOutItem::new("REFERENCE_2".to_string(), "MFR2".to_string(), "PART2".to_string()),
        ];

        // and
        let expected_results = Ok(vec![
            PlacementPartMappingResult {
                part: Some(&parts[2 - 1]),
                eda_placement: &eda_placements[0],
                mapping_result: Ok(vec![
                    PartMappingResult { part_mapping: &part_mappings[0], applied_rule: None },
                    PartMappingResult { part_mapping: &part_mappings[1], applied_rule: Some(AppliedMappingRule::FoundInLoadOut("REFERENCE_2".to_string())) },
                ])
            },
        ]);

        // when
        let matched_mappings = PartMapper::process(&eda_placements, &part_mappings, &load_out_items, &[]);

        // then
        assert_eq!(matched_mappings, expected_results);
    }

    #[test]
    fn map_parts_with_multiple_matching_mappings_with_an_assembly_rule() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME1".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE1".to_string()),
            ],
        };

        let eda_placements = vec![eda_placement1];

        // and
        let part1 = Part::new("MFR1".to_string(), "PART1".to_string());
        let part2 = Part::new("MFR2".to_string(), "PART2".to_string());
        let part3 = Part::new("MFR3".to_string(), "PART3".to_string());

        let parts = [part1, part2, part3];

        // and
        let criteria1 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME1".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE1".to_string() ),
        ]};
        let part_mapping1 = PartMapping::new(&parts[1 - 1], vec![Box::new(criteria1)]);
        let criteria2 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME1".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE1".to_string() ),
        ]};
        let part_mapping2 = PartMapping::new(&parts[2 - 1], vec![Box::new(criteria2)]);

        let part_mappings = vec![part_mapping1, part_mapping2];

        // and
        let assembly_rule1 = AssemblyRule {
            ref_des: "C1".to_string(),
            manufacturer: "MFR3".to_string(),
            mpn: "PART3".to_string(),
        };
        let assembly_rule2 = AssemblyRule {
            ref_des: "R1".to_string(),
            manufacturer: "MFR2".to_string(),
            mpn: "PART2".to_string(),
        };
        let assembly_rules = &[assembly_rule1, assembly_rule2];

        // and
        let expected_results = Ok(vec![
            PlacementPartMappingResult {
                part: Some(&parts[2-1]),
                eda_placement: &eda_placements[0],
                mapping_result: Ok(vec![
                    PartMappingResult { part_mapping: &part_mappings[0], applied_rule: None },
                    PartMappingResult { part_mapping: &part_mappings[1], applied_rule: Some(AppliedMappingRule::AssemblyRule) },
                ])
            },
        ]);

        // when
        let matched_mappings = PartMapper::process(&eda_placements, &part_mappings, &[], assembly_rules);

        // then
        assert_eq!(matched_mappings, expected_results);
    }

    #[test]
    fn map_parts_with_multiple_matching_mappings_with_an_assembly_rule_and_loadout_item() {
        // given
        let eda_placement1 = EdaPlacement { ref_des: "R1".to_string(), place: true,
            fields: vec![
                EdaPlacementField::new("name".to_string(), "NAME1".to_string()),
                EdaPlacementField::new("value".to_string(), "VALUE1".to_string()),
            ],
        };

        let eda_placements = vec![eda_placement1];

        // and
        let part1 = Part::new("MFR1".to_string(), "PART1".to_string());
        let part2 = Part::new("MFR2".to_string(), "PART2".to_string());

        let parts = [part1, part2];

        // and
        let criteria1 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME1".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE1".to_string() ),
        ]};
        let part_mapping1 = PartMapping::new(&parts[1 - 1], vec![Box::new(criteria1)]);
        let criteria2 = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem::new("name".to_string(), "NAME1".to_string() ),
            GenericCriteriaItem::new("value".to_string(), "VALUE1".to_string() ),
        ]};
        let part_mapping2 = PartMapping::new(&parts[2 - 1], vec![Box::new(criteria2)]);

        let part_mappings = vec![part_mapping1, part_mapping2];

        // and
        let load_out_items = vec![
            LoadOutItem::new("REFERENCE_1".to_string(), "MFR1".to_string(), "PART1".to_string()),
        ];

        // and
        let assembly_rule1 = AssemblyRule {
            ref_des: "R1".to_string(),
            manufacturer: "MFR2".to_string(),
            mpn: "PART2".to_string(),
        };
        let assembly_rules = &[assembly_rule1];

        // and
        let expected_results = Ok(vec![
            PlacementPartMappingResult {
                part: Some(&parts[2-1]),
                eda_placement: &eda_placements[0],
                mapping_result: Ok(vec![
                    PartMappingResult { part_mapping: &part_mappings[0], applied_rule: None },
                    PartMappingResult { part_mapping: &part_mappings[1], applied_rule: Some(AppliedMappingRule::AssemblyRule) },
                ])
            },
        ]);

        // when
        let matched_mappings = PartMapper::process(&eda_placements, &part_mappings, &load_out_items, assembly_rules);

        // then
        assert_eq!(matched_mappings, expected_results);
    }
}