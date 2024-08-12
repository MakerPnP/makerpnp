#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum PcbSide {
    Top,
    Bottom,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pcb {
    pub kind: PcbKind,
    pub name: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum PcbKind {
    Single,
    Panel,
}

impl TryFrom<&String> for PcbKind {
    type Error = ();

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "single" => Ok(PcbKind::Single),
            "panel" => Ok(PcbKind::Panel),
            _ => Err(())
        }
    }
}