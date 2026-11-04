//! The stores service's watching of CSV files (`STORE-9`).

use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use futures::executor::block_on;
use futures::StreamExt;
use stores_protocol::{PartsSource, StoresOperation, StoresOutput};
use stores_service::StoresService;

/// Long enough for the file system to say a file changed, even on a busy machine.
const CHANGE_TIMEOUT: Duration = Duration::from_secs(10);
/// How long to wait to be sure nothing is said.
const QUIET: Duration = Duration::from_millis(500);

fn csv_file(folder: &Path, name: &str, content: &str) -> PartsSource {
    let path = folder.join(name);
    fs::write(&path, content).unwrap();
    PartsSource::File(path)
}

/// Watches the data source, on a thread of its own, and passes on what the watch says.
fn watch(source: &PartsSource) -> mpsc::Receiver<StoresOutput> {
    let mut changes = StoresService::new().perform(StoresOperation::WatchSource {
        source: source.clone(),
    });
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        block_on(async move {
            while let Some(output) = changes.next().await {
                if sender.send(output).is_err() {
                    return;
                }
            }
        })
    });
    receiver
}

/// Waits for things to settle, and forgets anything said so far.
fn settle(changes: &mpsc::Receiver<StoresOutput>) {
    while changes.recv_timeout(QUIET).is_ok() {}
}

/// `STORE-9`
#[test]
fn says_when_the_file_is_written() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");
    let changes = watch(&source);
    settle(&changes);

    csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n",
    );

    assert_eq!(changes.recv_timeout(CHANGE_TIMEOUT), Ok(StoresOutput::Changed));
}

/// `STORE-9`, an editor may save a file by writing another, and renaming it over the first.
#[test]
fn says_when_the_file_is_replaced() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");
    let changes = watch(&source);
    settle(&changes);

    let replacement = folder.path().join("parts.csv.new");
    fs::write(&replacement, "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n").unwrap();
    fs::rename(&replacement, source.path().unwrap()).unwrap();

    assert_eq!(changes.recv_timeout(CHANGE_TIMEOUT), Ok(StoresOutput::Changed));
}

/// `STORE-9`, the service's own reads aren't changes, or it would read the file again and again.
#[test]
fn says_nothing_when_the_file_is_read() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");
    let changes = watch(&source);
    settle(&changes);

    let output = block_on(StoresService::new().perform_once(StoresOperation::ReadParts {
        source: source.clone(),
    }));
    assert!(matches!(output, Some(StoresOutput::PartsRead(Ok(_)))));

    assert_eq!(changes.recv_timeout(QUIET), Err(mpsc::RecvTimeoutError::Timeout));
}

/// `STORE-9`, only the file watched, not the others in its folder.
#[test]
fn says_nothing_when_another_file_in_the_folder_is_written() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");
    let changes = watch(&source);
    settle(&changes);

    csv_file(folder.path(), "other.csv", "\"Manufacturer\",\"Mpn\"\n");

    assert_eq!(changes.recv_timeout(QUIET), Err(mpsc::RecvTimeoutError::Timeout));
}
