use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

use anyhow::Error;
use as_any::AsAny;
use pnp::object_path::ObjectPath;
use pnp::reference::Reference;
use serde::Serialize;
use serde_json::Value;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use time::serde::rfc3339;
use time::OffsetDateTime;

use crate::placement::PlacementOperation;
use crate::process::{OperationReference, TaskKind, TaskReference, TaskStatus};

#[typetag::serde(tag = "type")]
pub trait OperationHistoryKind: AsAny + Debug {}

/// A task's status changed.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct TaskOperationHistoryKind {
    pub(crate) kind: TaskKind,
    pub(crate) status: TaskStatus,
}

#[typetag::serde(name = "task_operation")]
impl OperationHistoryKind for TaskOperationHistoryKind {}

// The kinds below are those of the tasks that were built into the planner before tasks had kinds.  They're only read
// from the histories of older projects.

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LoadPcbsOperationTaskHistoryKind {
    pub(crate) status: TaskStatus,
}

#[typetag::serde(name = "load_pcbs_operation")]
impl OperationHistoryKind for LoadPcbsOperationTaskHistoryKind {}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PlaceComponentsOperationTaskHistoryKind {
    pub(crate) status: TaskStatus,
}

#[typetag::serde(name = "place_components_operation")]
impl OperationHistoryKind for PlaceComponentsOperationTaskHistoryKind {}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ManualSolderingOperationTaskHistoryKind {
    pub(crate) status: TaskStatus,
}

#[typetag::serde(name = "manual_soldering_operation")]
impl OperationHistoryKind for ManualSolderingOperationTaskHistoryKind {}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct AutomatedSolderingOperationTaskHistoryKind {
    pub(crate) status: TaskStatus,
}

#[typetag::serde(name = "automated_soldering_operation")]
impl OperationHistoryKind for AutomatedSolderingOperationTaskHistoryKind {}

#[serde_as]
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PlacementOperationHistoryKind {
    #[serde_as(as = "DisplayFromStr")]
    pub object_path: ObjectPath,
    pub operation: PlacementOperation,
}

#[typetag::serde(name = "placement_operation")]
impl OperationHistoryKind for PlacementOperationHistoryKind {}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct OperationHistoryItem {
    #[serde(with = "rfc3339")]
    pub date_time: OffsetDateTime,
    pub phase: Reference,

    pub operation_reference: OperationReference,
    pub task_reference: TaskReference,
    pub task_history: Box<dyn OperationHistoryKind>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Items to add to a phase's operation history, each as its JSON, so they can be kept by a command, as a history item
/// can't be sent between threads (`STORE-13`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationHistoryAppend {
    /// The phase's operation history file, see [`phase_log_path`].
    pub path: PathBuf,
    pub items: Vec<String>,
}

impl OperationHistoryAppend {
    pub fn new(path: PathBuf, items: &[OperationHistoryItem]) -> Result<Self, Error> {
        let items = items
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            path,
            items,
        })
    }
}

/// The phase's operation history file, `<directory>/<phase>_log.json`.
pub fn phase_log_path(directory: &Path, phase: &Reference) -> PathBuf {
    directory.join(format!("{}_log.json", phase))
}

/// The operation history's contents, with the items added: the history's, or a new one's, if there's none.
///
/// Nothing is read or written, so a core can have the stores service read and write the history (`STORE-13`).
pub fn append(contents: Option<&str>, items: &[String]) -> Result<String, Error> {
    let mut operation_history: Vec<OperationHistoryItem> = match contents {
        Some(contents) => serde_json::from_str(contents)?,
        None => vec![],
    };
    for item in items {
        operation_history.push(serde_json::from_str(item)?);
    }

    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(vec![], formatter);
    operation_history.serialize(&mut ser)?;

    Ok(String::from_utf8(ser.into_inner())?)
}
