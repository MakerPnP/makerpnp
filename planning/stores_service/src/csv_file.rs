//! The back end for data sources that are CSV files, on a native file system.
//!
//! The files are read with `std::fs`, which blocks the thread the future is polled on, as the files are small.

use anyhow::{anyhow, Context};
use futures::channel::mpsc;
use futures::stream::{self, BoxStream};
use futures::StreamExt;
use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use stores_protocol::{
    Parts, PartsSource, PartsViewItem, Records, RecordsKind, RecordsOf, Revision, Source, StoresOutput,
};
use tracing::{info, warn};

/// For a file that exists, its canonical path, without the `\\?\` prefix Windows adds where it isn't needed, otherwise
/// the source as it is, so that an error names it as the user did (`STORE-6`).
///
/// For example `d:\parts\Parts.csv` and `D:\parts\..\parts\parts.csv` are the same file on Windows, so the same data
/// source.
pub fn resolve(source: &Source) -> Source {
    match source {
        Source::File(path) => dunce::canonicalize(path)
            .map(Source::File)
            .unwrap_or_else(|_| source.clone()),
        _ => source.clone(),
    }
}

/// The parts, and the revision of the file they were read from (`STORE-8`).
pub async fn read_parts(source: &PartsSource) -> anyhow::Result<Parts> {
    let Source::File(path) = source else {
        return Err(anyhow!("Unsupported source. source: {:?}", source));
    };

    info!("Loading parts. source: '{}'", source);
    // read once, so the revision is of the contents the parts were read from.
    let contents = std::fs::read(path).with_context(|| format!("Error reading parts. file: {}", path.display()))?;
    let revision = revision(&contents);

    let parts = stores::parts::read_parts(contents.as_slice())
        .with_context(|| format!("Error reading parts. file: {}", path.display()))?
        .into_iter()
        .map(|(part, fields)| PartsViewItem {
            part,
            fields: fields.into_iter().collect(),
        })
        .collect();

    Ok(Parts {
        revision,
        parts,
    })
}

/// A [`StoresOutput::Changed`] each time the file may have changed, by anyone, until the stream is dropped (`STORE-9`).
///
/// The file's folder is watched, not the file, as an editor may save a file by replacing it, which would end a watch on
/// the file itself. Only changes that could change its contents are passed on, not reads, such as the service's own,
/// which would otherwise have it read again and again. If the file can't be watched, the stream ends at once, and the
/// data source is only read again when asked.
pub fn watch(source: &Source) -> BoxStream<'static, StoresOutput> {
    let Source::File(path) = source else {
        return stream::empty().boxed();
    };
    let (Some(folder), Some(file_name)) = (path.parent(), path.file_name()) else {
        return stream::empty().boxed();
    };
    let file_name = file_name.to_owned();

    let (sender, receiver) = mpsc::unbounded::<()>();
    let watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| match result {
        Ok(event) if could_change_contents(&event.kind) => {
            if event
                .paths
                .iter()
                .any(|path| path.file_name() == Some(file_name.as_os_str()))
            {
                // the stream may have been dropped, in which case no one is watching.
                let _ = sender.unbounded_send(());
            }
        }
        Ok(_) => {}
        Err(cause) => warn!("Unable to watch the data source. cause: {:?}", cause),
    });
    let mut watcher = match watcher {
        Ok(watcher) => watcher,
        Err(cause) => {
            warn!(
                "Unable to watch the data source. source: '{}', cause: {:?}",
                source, cause
            );
            return stream::empty().boxed();
        }
    };
    if let Err(cause) = watcher.watch(folder, RecursiveMode::NonRecursive) {
        warn!(
            "Unable to watch the data source. source: '{}', cause: {:?}",
            source, cause
        );
        return stream::empty().boxed();
    }
    info!("Watching the data source. source: '{}'", source);

    // the watcher is kept with the stream, so it stops watching when the stream is dropped.
    stream::unfold((watcher, receiver), |(watcher, mut receiver)| async move {
        receiver
            .next()
            .await
            .map(|()| (StoresOutput::Changed, (watcher, receiver)))
    })
    .boxed()
}

fn could_change_contents(kind: &EventKind) -> bool {
    match kind {
        EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => true,
        EventKind::Access(_) | EventKind::Any | EventKind::Other => false,
    }
}

/// The revision of a file's contents: their SHA-256 hash, so it's the same in every process, and changes if they do.
pub fn revision(contents: &[u8]) -> Revision {
    Revision(format!("{:x}", Sha256::digest(contents)))
}

/// The data source's records, of the kind, and their revision, or `None` if there's no file (`STORE-11`).
pub async fn read_records(kind: RecordsKind, source: &Source) -> Result<Option<Records>, String> {
    let Source::File(path) = source else {
        return Err(format!("Unsupported source. source: {:?}", source));
    };

    info!("Loading {}. source: '{}'", name(kind), source);
    // read once, so the revision is of the contents the records were read from.
    let contents = match std::fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Unable to read the records. source: '{}', cause: '{}'",
                source, error
            ))
        }
    };
    let revision = revision(&contents);

    let records = match kind {
        RecordsKind::LoadOut => stores::load_out::read_items(contents.as_slice()).map(RecordsOf::LoadOut),
        RecordsKind::Packages => stores::packages::read_packages(contents.as_slice()).map(RecordsOf::Packages),
        RecordsKind::PackageMappings => {
            stores::package_mappings::read_package_mapping_rows(contents.as_slice()).map(RecordsOf::PackageMappings)
        }
        RecordsKind::Placements => stores::placements::read_placements(contents.as_slice()).map(RecordsOf::Placements),
        RecordsKind::EdaPlacements {
            eda_tool,
        } => stores::eda_placements::read_eda_placements(eda_tool, contents.as_slice()).map(|placements| {
            RecordsOf::EdaPlacements {
                eda_tool,
                placements,
            }
        }),
        RecordsKind::PartMappings => stores::csv::read_rows(contents.as_slice()).map(RecordsOf::PartMappings),
        RecordsKind::Substitutions => stores::csv::read_rows(contents.as_slice()).map(RecordsOf::Substitutions),
        RecordsKind::AssemblyRules => stores::csv::read_rows(contents.as_slice()).map(RecordsOf::AssemblyRules),
    }
    .map_err(|error| format!("Unable to read the records. source: '{}', cause: '{:#}'", source, error))?;

    Ok(Some(Records {
        revision,
        records,
    }))
}

/// Writes the records, replacing the file if there is one, and returns the revision written (`STORE-11`).
pub async fn write_records(source: &Source, records: &RecordsOf) -> Result<Revision, String> {
    let Source::File(path) = source else {
        return Err(format!("Unsupported source. source: {:?}", source));
    };

    info!("Storing {}. source: '{}'", name(records.kind()), source);
    let mut contents = vec![];
    match records {
        RecordsOf::LoadOut(items) => stores::load_out::write_items(&mut contents, items),
        records => Err(anyhow!("Unsupported records. kind: {:?}", records.kind())),
    }
    .map_err(|error| {
        format!(
            "Unable to write the records. source: '{}', cause: '{:#}'",
            source, error
        )
    })?;

    std::fs::write(path, &contents)
        .map_err(|error| format!("Unable to write the records. source: '{}', cause: '{}'", source, error))?;

    Ok(revision(&contents))
}

/// What the records are called in the logs, as they were when the core read them itself.
fn name(kind: RecordsKind) -> &'static str {
    match kind {
        RecordsKind::LoadOut => "load-out",
        RecordsKind::Packages => "packages",
        RecordsKind::PackageMappings => "package mappings",
        RecordsKind::Placements => "placements",
        RecordsKind::EdaPlacements {
            ..
        } => "eda placements",
        RecordsKind::PartMappings => "part mappings",
        RecordsKind::Substitutions => "eda substitutions",
        RecordsKind::AssemblyRules => "assembly rules",
    }
}
