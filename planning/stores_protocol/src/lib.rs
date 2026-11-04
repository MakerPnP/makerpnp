//! The store operations a core asks its shell for, as Crux effects, and their results (`STORE-7`).
//!
//! A core never reads or writes a data source itself. It asks for a [`StoresOperation`], the shell has the stores
//! service do it, and the core is given the [`StoresOutput`]. So the cores are pure, and are tested with outputs made
//! up by the tests, and every core in a process shares the one stores service.
//!
//! A data source, such as a parts file, is read and written by the service, which knows its format, so that it can have
//! back ends of other kinds, such as a database. A document, such as a project, is read and written as it is, by the
//! core that keeps it (`STORE-10`).

// A core does no IO, see `clippy.toml`.
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;

use crux_core::capability::Operation;
pub use eda::placement::EdaPlacement;
pub use eda::EdaTool;
pub use pnp::load_out::LoadOutItem;
pub use pnp::package::Package;
pub use pnp::part::Part;
pub use pnp::placement::Placement;
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

/// A document, such as a project, a PCB or a process, as its core keeps it, and the revision it was read from
/// (`STORE-10`).
///
/// The core that keeps the document reads and writes its contents, so the service doesn't need to know what's in it.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct Document {
    pub revision: Revision,
    pub contents: String,
}

/// Says how long the contents are, not what they are, as a document is logged, and may be large.
impl Debug for Document {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Document(<{} bytes>, revision: {:?})",
            self.contents.len(),
            self.revision.0
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
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

    /// The document at the path, if there is one (`STORE-10`). Answered with [`StoresOutput::DocumentRead`].
    ReadDocument { path: PathBuf },

    /// Writes the document's contents to the path, replacing any document there (`STORE-10`). Answered with
    /// [`StoresOutput::DocumentWritten`].
    WriteDocument { path: PathBuf, contents: String },

    /// The records of a data source of the kind, such as a load-out, if there is one (`STORE-11`). Answered with
    /// [`StoresOutput::RecordsRead`].
    ReadRecords { kind: RecordsKind, source: Source },

    /// Writes the records, replacing the data source's (`STORE-11`). Answered with [`StoresOutput::RecordsWritten`].
    WriteRecords { source: Source, records: RecordsOf },

    /// Writes the parts, replacing the data source's, only if it's still at the revision the change was based on, so
    /// that a change made since, by anyone, isn't lost (`STORE-15`). Answered with [`StoresOutput::PartsWritten`].
    WriteParts {
        source: PartsSource,
        revision: Revision,
        parts: Vec<PartsViewItem>,
    },
}

/// Why parts weren't written (`STORE-15`).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum PartsWriteError {
    /// The data source has changed since the revision the change was based on, to this revision, or `None` if it no
    /// longer exists, so the change would lose another's.
    Changed { revision: Option<Revision> },
    /// The parts couldn't be written, for example as the file is read-only.
    Failed { cause: String },
}

/// The kinds of data source whose records a core reads, each with a format the service knows (`STORE-11`).
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord
)]
pub enum RecordsKind {
    LoadOut,
    Packages,
    /// Each record's fields by name, as each mapping is made with the packages, by the core.
    PackageMappings,
    Placements,
    /// The placements from an EDA tool's file, whose format depends on the tool.
    EdaPlacements {
        eda_tool: EdaTool,
    },
    /// Each record's fields by name, as each mapping is made with the parts, by the core.
    PartMappings,
    /// Each record's fields by name, as each rule is made by the core.
    Substitutions,
    /// Each record's fields by name, as each rule is made by the core.
    AssemblyRules,
}

/// A data source's records, of its kind.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum RecordsOf {
    LoadOut(Vec<LoadOutItem>),
    Packages(Vec<Package>),
    PackageMappings(Vec<BTreeMap<String, String>>),
    Placements(Vec<Placement>),
    EdaPlacements {
        eda_tool: EdaTool,
        placements: Vec<EdaPlacement>,
    },
    PartMappings(Vec<BTreeMap<String, String>>),
    Substitutions(Vec<BTreeMap<String, String>>),
    AssemblyRules(Vec<BTreeMap<String, String>>),
}

impl RecordsOf {
    pub fn kind(&self) -> RecordsKind {
        match self {
            RecordsOf::LoadOut(_) => RecordsKind::LoadOut,
            RecordsOf::Packages(_) => RecordsKind::Packages,
            RecordsOf::PackageMappings(_) => RecordsKind::PackageMappings,
            RecordsOf::Placements(_) => RecordsKind::Placements,
            RecordsOf::EdaPlacements {
                eda_tool, ..
            } => RecordsKind::EdaPlacements {
                eda_tool: *eda_tool,
            },
            RecordsOf::PartMappings(_) => RecordsKind::PartMappings,
            RecordsOf::Substitutions(_) => RecordsKind::Substitutions,
            RecordsOf::AssemblyRules(_) => RecordsKind::AssemblyRules,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            RecordsOf::LoadOut(records) => records.len(),
            RecordsOf::Packages(records) => records.len(),
            RecordsOf::PackageMappings(records) => records.len(),
            RecordsOf::Placements(records) => records.len(),
            RecordsOf::EdaPlacements {
                placements, ..
            } => placements.len(),
            RecordsOf::PartMappings(records) => records.len(),
            RecordsOf::Substitutions(records) => records.len(),
            RecordsOf::AssemblyRules(records) => records.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Says how many records there are, of what kind, not what they are, as they're logged.
struct RecordsSummary<'a>(&'a RecordsOf);

impl Debug for RecordsSummary<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{} {:?} records>", self.0.len(), self.0.kind())
    }
}

/// A data source's records, and the revision they were read from (`STORE-8`).
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct Records {
    pub revision: Revision,
    pub records: RecordsOf,
}

impl Debug for Records {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Records({:?}, revision: {:?})",
            RecordsSummary(&self.records),
            self.revision.0
        )
    }
}

/// Says how long a document written is, not what it is, as an operation is logged.
impl Debug for StoresOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StoresOperation::ResolveSource {
                source,
            } => f
                .debug_struct("ResolveSource")
                .field("source", source)
                .finish(),
            StoresOperation::ReadParts {
                source,
            } => f
                .debug_struct("ReadParts")
                .field("source", source)
                .finish(),
            StoresOperation::WatchSource {
                source,
            } => f
                .debug_struct("WatchSource")
                .field("source", source)
                .finish(),
            StoresOperation::ReadDocument {
                path,
            } => f
                .debug_struct("ReadDocument")
                .field("path", path)
                .finish(),
            StoresOperation::WriteDocument {
                path,
                contents,
            } => write!(
                f,
                "WriteDocument {{ path: {:?}, contents: <{} bytes> }}",
                path,
                contents.len()
            ),
            StoresOperation::ReadRecords {
                kind,
                source,
            } => f
                .debug_struct("ReadRecords")
                .field("kind", kind)
                .field("source", source)
                .finish(),
            StoresOperation::WriteRecords {
                source,
                records,
            } => f
                .debug_struct("WriteRecords")
                .field("source", source)
                .field("records", &RecordsSummary(records))
                .finish(),
            StoresOperation::WriteParts {
                source,
                revision,
                parts,
            } => write!(
                f,
                "WriteParts {{ source: {:?}, revision: {:?}, parts: <{} parts> }}",
                source,
                revision.0,
                parts.len()
            ),
        }
    }
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

    /// The document, or `None` if there's no document at the path, or why it couldn't be read.
    DocumentRead(Result<Option<Document>, String>),

    /// The revision of the document written, or why it couldn't be written.
    DocumentWritten(Result<Revision, String>),

    /// The records, or `None` if there's no data source, for example no file, or why they couldn't be read.
    RecordsRead(Result<Option<Records>, String>),

    /// The revision of the records written, or why they couldn't be written.
    RecordsWritten(Result<Revision, String>),

    /// The revision of the parts written, or why they weren't.
    PartsWritten(Result<Revision, PartsWriteError>),
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
            StoresOutput::DocumentRead(result) => write!(f, "DocumentRead({:?})", result),
            StoresOutput::DocumentWritten(result) => write!(f, "DocumentWritten({:?})", result),
            StoresOutput::RecordsRead(result) => write!(f, "RecordsRead({:?})", result),
            StoresOutput::RecordsWritten(result) => write!(f, "RecordsWritten({:?})", result),
            StoresOutput::PartsWritten(result) => write!(f, "PartsWritten({:?})", result),
        }
    }
}
