use serde_json::Value;
use std::collections::HashMap;
use time::serde::rfc3339;
use time::{OffsetDateTime};
use crate::common::project_builder::TestProcessOperationStatus;

#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub enum TestOperationHistoryPlacementOperation {
    Placed
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub enum TestOperationHistoryKind {
    LoadPcbs { status: TestProcessOperationStatus },
    // FUTURE add support for other kinds that can be used, see `OperationHistoryKind` 
    PlacementOperation { object_path: String, operation: TestOperationHistoryPlacementOperation },
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
