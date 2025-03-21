use futures::stream::BoxStream;
use futures::Stream;
use std::pin::Pin;
use tokio::runtime::Runtime;

pub fn boxed_stream<T>(stream: impl Stream<Item = T> + Send + 'static) -> BoxStream<'static, T> {
    Box::pin(stream) as Pin<Box<dyn Stream<Item = T> + Send>>
}

/// Global Tokio runtime for the application.
/// This is used for running async tasks and managing the event loop.
pub struct TokioRuntime {
    runtime: Runtime,
}

impl TokioRuntime {
    /// Creates a new Tokio runtime.
    pub fn new() -> Self {
        let runtime = Runtime::new().expect("Failed to create Tokio runtime");
        Self { runtime }
    }

    /// Gets a reference to the Tokio runtime.
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
}

impl Default for TokioRuntime {
    fn default() -> Self {
        Self::new()
    }
}
