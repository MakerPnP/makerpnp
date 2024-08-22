use serde_json::Value;
use std::collections::HashMap;
use time::serde::rfc3339;
use time::{OffsetDateTime};

#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub enum TestOperationHistoryKind {
    LoadPcbs { completed: bool },
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TestOperationHistoryItem {
    #[serde(with = "rfc3339")]
    pub date_time: OffsetDateTime,
    pub phase: String,

    pub operation: TestOperationHistoryKind,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>
}
