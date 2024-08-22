use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use anyhow::Error;
use serde::Serialize;
use serde_json::Value;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use time::serde::rfc3339;
use time::OffsetDateTime;
use tracing::info;
use crate::planning::placement::PlacementOperation;
use crate::planning::reference::Reference;
use crate::pnp::object_path::ObjectPath;

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum OperationHistoryKind {
    LoadPcbs { completed: bool },
    AutomatedPnp,
    ReflowComponents,
    ManuallySolderComponents,
    PlacementOperation {
        #[serde_as(as = "DisplayFromStr")]
        object_path: ObjectPath,
        operation: PlacementOperation
    },
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

pub fn read_or_default(phase_log_path: &PathBuf) -> Result<Vec<OperationHistoryItem>, Error> {
    let is_new = !phase_log_path.exists();
    if is_new {
        return Ok(Default::default());
    }

    // TODO use a context for better error messages
    let file = File::open(phase_log_path.clone())?;

    let operation_history = serde_json::from_reader(file)?;
    
    Ok(operation_history)
}