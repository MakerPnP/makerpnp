use crate::eda::placement::{EdaPlacement};
use crate::part_mapper::criteria::PlacementMappingCriteria;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericCriteriaItem {
    pub field_name: String,
    // TODO replace with exact-match/regexp-match/etc, for now exact-match only.
    pub field_pattern: String,
}

impl GenericCriteriaItem {
    pub fn new(field_name: String, field_pattern: String) -> Self {
        Self {
            field_name,
            field_pattern
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericExactMatchCriteria {
    pub criteria: Vec<GenericCriteriaItem>,
}

impl PlacementMappingCriteria for GenericExactMatchCriteria {

    fn matches(&self, eda_placement: &EdaPlacement) -> bool {

        let result: Option<bool> = self.criteria.iter().fold(None, |mut matched, ref criterion| {
            let matched_field = eda_placement.fields.iter().find(|field | {
                criterion.field_name.eq(field.name.as_str()) &&
                    criterion.field_pattern.eq(field.value.as_str())
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
}

#[cfg(test)]
mod exact_match_critera_tests {
    use crate::eda::criteria::{GenericCriteriaItem, GenericExactMatchCriteria};
    use crate::eda::placement::{EdaPlacement, EdaPlacementField};
    use crate::part_mapper::criteria::PlacementMappingCriteria;

    #[test]
    fn matches() {
        // given
        let criteria = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem { field_name: "name".to_string(), field_pattern: "NAME1".to_string() },
            GenericCriteriaItem { field_name: "value".to_string(), field_pattern: "VALUE1".to_string() },
        ]};
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField { name: "name".to_string(), value: "NAME1".to_string() },
                EdaPlacementField { name: "value".to_string(), value: "VALUE1".to_string() },
            ],
        };

        // when
        assert!(criteria.matches(&placement));
    }

    #[test]
    fn does_not_match_due_to_name() {
        // given
        let criteria = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem { field_name: "name".to_string(), field_pattern: "NAME1".to_string() },
            GenericCriteriaItem { field_name: "value".to_string(), field_pattern: "VALUE1".to_string() },
        ]};
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField { name: "name".to_string(), value: "NAME2".to_string() },
                EdaPlacementField { name: "value".to_string(), value: "VALUE1".to_string() },
            ],
        };

        // when
        assert!(!criteria.matches(&placement));
    }

    #[test]
    fn does_not_match_due_to_value() {
        // given
        let criteria = GenericExactMatchCriteria { criteria: vec![
            GenericCriteriaItem { field_name: "name".to_string(), field_pattern: "NAME1".to_string() },
            GenericCriteriaItem { field_name: "value".to_string(), field_pattern: "VALUE1".to_string() },
        ]};
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            place: true,
            fields: vec![
                EdaPlacementField { name: "name".to_string(), value: "NAME2".to_string() },
                EdaPlacementField { name: "value".to_string(), value: "VALUE2".to_string() },
            ],
        };

        // when
        assert!(!criteria.matches(&placement));
    }
}
