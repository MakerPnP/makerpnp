use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SortOrder {
    Asc,
    Desc,
}

impl Display for SortOrder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asc=> write!(f, "Asc"),
            Self::Desc=> write!(f, "Desc"),
        }
    }
}