use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use anyhow::Error;
use serde::Serialize;
use time::serde::rfc3339;
use time::OffsetDateTime;
use tracing::info;
use crate::planning::reference::Reference;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum OperationHistoryKind {
    LoadPcbs { completed: bool },
    AutomatedPnp,
    ReflowComponents,
    ManuallySolderComponents,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OperationHistoryItem {
    #[serde(with = "rfc3339")]
    pub date_time: OffsetDateTime,
    pub phase: Reference,
    pub operation: OperationHistoryKind,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>
}

pub fn write(phase_log_path: PathBuf, operation_history: &Vec<OperationHistoryItem>) -> Result<(), Error> {
    // TODO use a context for better error messages
    let is_new = !phase_log_path.exists();
    
    let file = File::create(phase_log_path.clone())?;

    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(file, formatter);
    operation_history.serialize(&mut ser)?;

    match is_new {
        true => info!("Created operation history file. path: {:?}\n", phase_log_path),
        false => info!("Updated operation history file. path: {:?}\n", phase_log_path),
    }
    
    Ok(())
}
