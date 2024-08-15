use std::fmt::{Debug, Display, Formatter};
use regex::Regex;
use crate::eda::placement::{EdaPlacement};
use crate::part_mapper::criteria::PlacementMappingCriteria;
use crate::util::dynamic::as_any::AsAny;
use crate::util::dynamic::dynamic_eq::DynamicEq;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactMatchCriterion {
    pub field_name: String,
    pub field_pattern: String,
}

impl ExactMatchCriterion {
    pub fn new(field_name: String, field_pattern: String) -> Self {
        Self {
            field_name,
            field_pattern
        }
    }
}

impl Display for ExactMatchCriterion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_pattern: '{}'", self.field_name, self.field_pattern)
    }
}

impl FieldCriterion for ExactMatchCriterion {
    fn matches(&self, name: &str, value: &str) -> bool {
        self.field_name.eq(name) &&
            self.field_pattern.eq(value) 
    }
}

#[cfg(test)]
mod exact_match_criterion_tests {
    use crate::eda::criteria::{ExactMatchCriterion, FieldCriterion};

    #[test]
    pub fn matches() {
        // given
        let criterion = ExactMatchCriterion { field_name: "name".to_string(), field_pattern: "NAME1".to_string() };

        // expect
        assert!(criterion.matches("name", "NAME1"))
    }
}


#[derive(Clone, Debug)]
pub struct RegexMatchCriterion {
    pub field_name: String,
    pub field_pattern: Regex,
}

impl Eq for RegexMatchCriterion {}

impl PartialEq for RegexMatchCriterion {
    fn eq(&self, other: &Self) -> bool {
        self.field_name.eq(&other.field_name) &&
            self.field_pattern.to_string().eq(&other.field_pattern.to_string())
    }
}

impl RegexMatchCriterion {
    pub fn new(field_name: String, field_pattern: Regex) -> Self {
        Self {
            field_name,
            field_pattern
        }
    }
}

impl Display for RegexMatchCriterion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_pattern: '{}'", self.field_name, self.field_pattern)
    }
}

impl FieldCriterion for RegexMatchCriterion {
    fn matches(&self, name: &str, value: &str) -> bool {
        self.field_name.eq(name) &&
            self.field_pattern.is_match(value)
    }
}

#[cfg(test)]
mod regex_match_criterion_tests {
    use regex::Regex;
    use crate::eda::criteria::{RegexMatchCriterion, FieldCriterion};

    #[test]
    pub fn matches() {
        // given
        let criterion = RegexMatchCriterion { field_name: "name".to_string(), field_pattern: Regex::new(".*").unwrap() };

        // expect
        assert!(criterion.matches("name", "ANTHING"))
    }
}

impl PartialEq for dyn FieldCriterion
{
    fn eq(&self, other: &Self) -> bool {
        self.dynamic_eq(other.as_any())
    }
}

pub trait FieldCriterion: Display + Debug + AsAny + DynamicEq {
    fn matches(&self, name: &str, value: &str) -> bool;
}

#[derive(Debug, PartialEq)]
pub struct GenericCriteria {
    pub criteria: Vec<Box<dyn FieldCriterion>>,
}

impl PlacementMappingCriteria for GenericCriteria {

    fn matches(&self, eda_placement: &EdaPlacement) -> bool {

        let result: Option<bool> = self.criteria.iter().fold(None, |mut matched, criterion| {
            let matched_field = eda_placement.fields.iter().find(|field | {
                criterion.matches(field.name.as_str(), field.value.as_str())
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
mod generic_criteria_tests {
    use regex::Regex;
    use crate::eda::criteria::{ExactMatchCriterion, GenericCriteria, RegexMatchCriterion};
    use crate::eda::placement::{EdaPlacement, EdaPlacementField};
    use crate::part_mapper::criteria::PlacementMappingCriteria;

    #[test]
    fn matches() {
        // given
        let criteria = GenericCriteria { criteria: vec![
            Box::new(ExactMatchCriterion { field_name: "name".to_string(), field_pattern: "NAME1".to_string() }),
            Box::new(RegexMatchCriterion { field_name: "value".to_string(), field_pattern: Regex::new(".*").unwrap() }),
        ]};
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            fields: vec![
                EdaPlacementField { name: "name".to_string(), value: "NAME1".to_string() },
                EdaPlacementField { name: "value".to_string(), value: "VALUE1".to_string() },
            ],
            ..EdaPlacement::default()
        };

        // when
        assert!(criteria.matches(&placement));
    }

    #[test]
    fn does_not_match_due_to_name() {
        // given
        let criteria = GenericCriteria { criteria: vec![
            Box::new(ExactMatchCriterion { field_name: "name".to_string(), field_pattern: "NAME1".to_string() }),
            Box::new(RegexMatchCriterion { field_name: "value".to_string(), field_pattern: Regex::new(".*").unwrap() }),
        ]};
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            fields: vec![
                EdaPlacementField { name: "name".to_string(), value: "NAME2".to_string() },
                EdaPlacementField { name: "value".to_string(), value: "VALUE1".to_string() },
            ],
            ..EdaPlacement::default()
        };

        // when
        assert!(!criteria.matches(&placement));
    }

    #[test]
    fn does_not_match_due_to_value() {
        // given
        let criteria = GenericCriteria { criteria: vec![
            Box::new(ExactMatchCriterion { field_name: "name".to_string(), field_pattern: "NAME1".to_string() }),
            Box::new(RegexMatchCriterion { field_name: "value".to_string(), field_pattern: Regex::new("(VALUE1)").unwrap() }),
        ]};
        let placement = EdaPlacement {
            ref_des: "R1".to_string(),
            fields: vec![
                EdaPlacementField { name: "name".to_string(), value: "NAME2".to_string() },
                EdaPlacementField { name: "value".to_string(), value: "VALUE2".to_string() },
            ],
            ..EdaPlacement::default()
        };

        // when
        assert!(!criteria.matches(&placement));
    }
}
