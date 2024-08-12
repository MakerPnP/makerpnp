use std::collections::BTreeSet;
use crate::planning::process::Process;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
#[derive(PartialEq, Eq)]
pub struct PartState {
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    #[serde(default)]
    pub applicable_processes: BTreeSet<Process>,
}