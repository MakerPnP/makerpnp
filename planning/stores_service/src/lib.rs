//! The stores service: does the store operations the cores ask their shell for (`STORE-7`).
//!
//! Every shell, the CLI and the GUI, has one, so that the cores' store operations are done in one place, and not by
//! each shell. Each process has one, shared by all of its cores. It's async, and not tied to a thread, so that it can be
//! used by a shell that has no threads, such as a web shell, which would have back ends of its own.
//!
//! Each back end is a module, chosen by the data source, for example [`csv_file`] for a file, or by the document, for
//! example [`document_file`].

use futures::stream::{self, BoxStream};
use futures::StreamExt;
use stores_protocol::{StoresOperation, StoresOutput};
use tracing::debug;

pub mod csv_file;
pub mod document_file;

/// A handle to the process's stores service: its clones are the same service, so a shell can give one to each of its
/// cores.
#[derive(Debug, Default, Clone)]
pub struct StoresService {}

impl StoresService {
    pub fn new() -> Self {
        Self {}
    }

    /// Does the operation, and returns what the core that asked for it is given: one result, or, for an operation
    /// that's a stream, such as [`StoresOperation::WatchSource`], a result each time there is one, until the stream is
    /// dropped (`StoresOperation::is_stream`).
    ///
    /// The stream owns what it needs, so it can be polled on any thread, or none.
    pub fn perform(&self, operation: StoresOperation) -> BoxStream<'static, StoresOutput> {
        // `LOG-3`, what the shell was asked to do for a core.
        debug!("Performing stores operation. operation: {:?}", operation);

        match operation {
            StoresOperation::ResolveSource {
                source,
            } => stream::once(async move {
                StoresOutput::SourceResolved {
                    source: csv_file::resolve(&source),
                }
            })
            .boxed(),
            StoresOperation::ReadParts {
                source,
            } => stream::once(async move {
                StoresOutput::PartsRead(
                    csv_file::read_parts(&source)
                        .await
                        // `{:#}` includes each cause, for example which header is missing.
                        .map_err(|error| format!("{:#}", error)),
                )
            })
            .boxed(),
            StoresOperation::WatchSource {
                source,
            } => csv_file::watch(&source),
            StoresOperation::ReadDocument {
                path,
            } => stream::once(async move { StoresOutput::DocumentRead(document_file::read(&path).await) }).boxed(),
            StoresOperation::WriteDocument {
                path,
                contents,
            } => {
                stream::once(async move { StoresOutput::DocumentWritten(document_file::write(&path, &contents).await) })
                    .boxed()
            }
            StoresOperation::ReadRecords {
                kind,
                source,
            } => stream::once(async move { StoresOutput::RecordsRead(csv_file::read_records(kind, &source).await) })
                .boxed(),
            StoresOperation::WriteRecords {
                source,
                records,
            } => stream::once(
                async move { StoresOutput::RecordsWritten(csv_file::write_records(&source, &records).await) },
            )
            .boxed(),
            StoresOperation::WriteParts {
                source,
                revision,
                parts,
            } => stream::once(async move {
                StoresOutput::PartsWritten(csv_file::write_parts(&source, &revision, &parts).await)
            })
            .boxed(),
            StoresOperation::ReadTable {
                kind,
                source,
            } => {
                stream::once(async move { StoresOutput::TableRead(csv_file::read_table(kind, &source).await) }).boxed()
            }
        }
    }

    /// Does an operation that has one result, see [`Self::perform`].
    pub async fn perform_once(&self, operation: StoresOperation) -> Option<StoresOutput> {
        debug_assert!(!operation.is_stream());
        self.perform(operation).next().await
    }
}
