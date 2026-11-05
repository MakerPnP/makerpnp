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
            field_pattern,
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
        self.field_name.eq(name) && self.field_pattern.eq(value)
    }
}

#[cfg(test)]
mod exact_match_criterion_tests {
    use crate::{ExactMatchCriterion, FieldCriterion};

    #[test]
    pub fn matches() {
        // given
        let criterion = ExactMatchCriterion {
            field_name: "name".to_string(),
            field_pattern: "NAME1".to_string(),
        };

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
        self.field_name.eq(&other.field_name)
            && self
                .field_pattern
                .to_string()
                .eq(&other.field_pattern.to_string())
    }
}

impl RegexMatchCriterion {
    pub fn new(field_name: String, field_pattern: Regex) -> Self {
        Self {
            field_name,
            field_pattern,
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
        self.field_name.eq(name) && self.field_pattern.is_match(value)
    }
}

#[cfg(test)]
mod regex_match_criterion_tests {
    use regex::Regex;

    use crate::{FieldCriterion, RegexMatchCriterion};

    #[test]
    pub fn matches() {
        // given
        let criterion = RegexMatchCriterion {
            field_name: "name".to_string(),
            field_pattern: Regex::new(".*").unwrap(),
        };

        // expect
        assert!(criterion.matches("name", "ANTHING"))
    }
}

impl PartialEq for dyn FieldCriterion {
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

/// A value a field's value is matched with, as a data source has it: a regular expression between `/`s, such as
/// `/RES_.*/`, or else the value itself, matched exactly.
///
/// A regular expression is parsed once, as the pattern is made, so it's ready to be used, and shown, as often as needed,
/// with why it isn't valid, if it isn't. A pattern is serialized as the data source has it, and parsed again when it's
/// deserialized.
#[derive(Clone, Debug)]
pub enum Pattern {
    Exact(String),
    Regex {
        /// As the data source has it, with its `/`s.
        text: String,
        regex: Result<Regex, regex::Error>,
    },
}

impl Pattern {
    pub fn parse(value: &str) -> Self {
        if value.starts_with('/') && value.ends_with('/') {
            // a lone `/` starts and ends with one, so it's an empty expression, which matches everything.
            let expression = value
                .get(1..value.len() - 1)
                .unwrap_or_default();
            Pattern::Regex {
                text: value.to_string(),
                regex: Regex::new(expression),
            }
        } else {
            Pattern::Exact(value.to_string())
        }
    }

    /// The pattern as the data source has it.
    pub fn text(&self) -> &str {
        match self {
            Pattern::Exact(text) => text,
            Pattern::Regex {
                text, ..
            } => text,
        }
    }
}

impl Display for Pattern {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.text())
    }
}

/// Patterns are the same if the data source has them the same.
impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.text() == other.text()
    }
}

impl serde::Serialize for Pattern {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.text())
    }
}

impl<'de> serde::Deserialize<'de> for Pattern {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(Pattern::parse(&text))
    }
}

/// A field's pattern, by the field's name, such as `name`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct FieldPattern {
    pub field_name: String,
    pub pattern: Pattern,
}

#[cfg(test)]
mod pattern_tests {
    use crate::Pattern;

    #[test]
    pub fn a_value_between_slashes_is_a_regular_expression() {
        let Pattern::Regex {
            text,
            regex,
        } = Pattern::parse("/RES_.*/")
        else {
            panic!("a regular expression");
        };
        assert_eq!(text, "/RES_.*/");
        assert!(regex.unwrap().is_match("RES_0402"));
    }

    #[test]
    pub fn an_invalid_regular_expression_says_why() {
        let Pattern::Regex {
            text,
            regex,
        } = Pattern::parse("/RES_[/")
        else {
            panic!("a regular expression");
        };
        assert_eq!(text, "/RES_[/");
        assert!(regex.is_err());
    }

    #[test]
    pub fn any_other_value_is_matched_exactly() {
        for value in ["RES_0402", "/RES_0402", "RES_0402/", ""] {
            assert_eq!(Pattern::parse(value), Pattern::Exact(value.to_string()), "{value}");
            assert!(matches!(Pattern::parse(value), Pattern::Exact(_)), "{value}");
        }
    }

    #[test]
    pub fn a_lone_slash_is_an_empty_regular_expression_that_matches_everything() {
        let Pattern::Regex {
            regex, ..
        } = Pattern::parse("/")
        else {
            panic!("a regular expression");
        };
        assert!(regex.unwrap().is_match("ANYTHING"));
    }

    #[test]
    pub fn a_pattern_is_serialized_as_the_data_source_has_it_and_parsed_again() {
        let pattern = Pattern::parse("/RES_.*/");
        let json = serde_json::to_string(&pattern).unwrap();
        assert_eq!(json, "\"/RES_.*/\"");

        let deserialized: Pattern = serde_json::from_str(&json).unwrap();
        assert!(matches!(&deserialized, Pattern::Regex {
            regex: Ok(_),
            ..
        }));
        assert_eq!(deserialized, pattern);
    }
}
