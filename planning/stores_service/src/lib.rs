//! The stores service: does the store operations the cores ask their shell for (`STORE-7`).
//!
//! Every shell, the CLI and the GUI, has one, so that the cores' store operations are done in one place, and not by
//! each shell. Each process has one, shared by all of its cores. It's async, and not tied to a thread, so that it can be
//! used by a shell that has no threads, such as a web shell, which would have back ends of its own.
//!
//! Each back end is a module, chosen by the data source, for example [`csv_file`] for a file.

use stores_protocol::{StoresOperation, StoresOutput};
use tracing::debug;

pub mod csv_file;

#[derive(Debug, Default)]
pub struct StoresService {}

impl StoresService {
    pub fn new() -> Self {
        Self {}
    }

    /// Does the operation, and returns what the core that asked for it is given.
    pub async fn perform(&self, operation: StoresOperation) -> StoresOutput {
        // `LOG-3`, what the shell was asked to do for a core.
        debug!("Performing stores operation. operation: {:?}", operation);

        match operation {
            StoresOperation::ResolveSource {
                source,
            } => StoresOutput::SourceResolved {
                source: csv_file::resolve(&source),
            },
            StoresOperation::ReadParts {
                source,
            } => StoresOutput::PartsRead(
                csv_file::read_parts(&source)
                    .await
                    // `{:#}` includes each cause, for example which header is missing.
                    .map_err(|error| format!("{:#}", error)),
            ),
        }
    }
}
