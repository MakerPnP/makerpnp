//! The store operations a core asks its shell for, as Crux effects, and their results (`STORE-7`).
//!
//! A core never reads or writes a data source itself. It asks for a [`StoresOperation`], the shell has the stores
//! service do it, and the core is given the [`StoresOutput`]. So the cores are pure, and are tested with outputs made
//! up by the tests, and every core in a process shares the one stores service.

use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};

use crux_core::capability::Operation;
pub use pnp::part::Part;
pub use util::source::Source;

pub type PartsSource = Source;

/// A part, and the data source's other values for it, for example a supplier's part code.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PartsViewItem {
    pub part: Part,
    /// The data source's other values for the part, by name, for example `"LCSC"`.
    pub fields: BTreeMap<String, String>,
}

/// Which version of a data source's contents something was read from, or based on (`STORE-8`).
///
/// Made by the back end, and only compared: two reads of a data source have the same revision only if its contents
/// were the same, in any process. For a file, it's a hash of the file's contents.
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord
)]
pub struct Revision(pub String);

impl Display for Revision {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The parts of a parts data source, and the revision they were read from.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Parts {
    pub revision: Revision,
    /// In the order the data source has them.
    pub parts: Vec<PartsViewItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum StoresOperation {
    /// The data source as the stores service identifies it, so that one file is one data source however its path is
    /// written (`STORE-6`). Answered with [`StoresOutput::SourceResolved`].
    ResolveSource { source: Source },

    /// The parts of a parts data source, in the order it has them. Answered with [`StoresOutput::PartsRead`].
    ReadParts { source: PartsSource },

    /// Watches the data source, as the stores service identified it, for changes, by anyone, for example another
    /// process (`STORE-9`). A stream: answered with [`StoresOutput::Changed`] each time it may have changed, for as long
    /// as the shell can watch it. A shell that can't, such as the CLI, which is done before anything could change,
    /// doesn't answer it.
    WatchSource { source: Source },
}

impl StoresOperation {
    /// Whether the operation is answered more than once, see [`StoresOperation::WatchSource`].
    pub fn is_stream(&self) -> bool {
        matches!(self, StoresOperation::WatchSource { .. })
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum StoresOutput {
    /// For a file, its canonical path, or the source as it is if the file doesn't exist.
    SourceResolved { source: Source },

    /// The parts, and the revision they were read from, or why they couldn't be read, for example which header is
    /// missing.
    PartsRead(Result<Parts, String>),

    /// The data source watched may have changed. It may not have, for example when a file was written with the same
    /// contents, which its revision shows once it's read again (`STORE-8`).
    Changed,
}

impl Operation for StoresOperation {
    type Output = StoresOutput;
}

/// Says how many parts were read, not what they are, as an output is logged, and may have thousands.
impl Debug for StoresOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StoresOutput::SourceResolved {
                source,
            } => f
                .debug_struct("SourceResolved")
                .field("source", source)
                .finish(),
            StoresOutput::PartsRead(Ok(Parts {
                revision,
                parts,
            })) => write!(f, "PartsRead(Ok(<{} parts>, revision: {:?}))", parts.len(), revision.0),
            StoresOutput::PartsRead(Err(error)) => write!(f, "PartsRead(Err({:?}))", error),
            StoresOutput::Changed => f.write_str("Changed"),
        }
    }
}
