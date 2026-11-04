//! The store operations a core asks its shell for, as Crux effects, and their results (`STORE-7`).
//!
//! A core never reads or writes a data source itself. It asks for a [`StoresOperation`], the shell has the stores
//! service do it, and the core is given the [`StoresOutput`]. So the cores are pure, and are tested with outputs made
//! up by the tests, and every core in a process shares the one stores service.

use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};

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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum StoresOperation {
    /// The data source as the stores service identifies it, so that one file is one data source however its path is
    /// written (`STORE-6`). Answered with [`StoresOutput::SourceResolved`].
    ResolveSource { source: Source },

    /// The parts of a parts data source, in the order it has them. Answered with [`StoresOutput::PartsRead`].
    ReadParts { source: PartsSource },
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum StoresOutput {
    /// For a file, its canonical path, or the source as it is if the file doesn't exist.
    SourceResolved { source: Source },

    /// The parts, or why they couldn't be read, for example which header is missing.
    PartsRead(Result<Vec<PartsViewItem>, String>),
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
            StoresOutput::PartsRead(Ok(parts)) => write!(f, "PartsRead(Ok(<{} parts>))", parts.len()),
            StoresOutput::PartsRead(Err(error)) => write!(f, "PartsRead(Err({:?}))", error),
        }
    }
}
