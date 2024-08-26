use std::fmt::{Debug, Display, Formatter};
use regex::Regex;
use util::dynamic::as_any::AsAny;
use util::dynamic::dynamic_eq::DynamicEq;

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
    use crate::{ExactMatchCriterion, FieldCriterion};

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
    use crate::{RegexMatchCriterion, FieldCriterion};

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
