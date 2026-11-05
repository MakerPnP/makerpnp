//! The back end for data sources that are CSV files, on a native file system.
//!
//! The files are read with `std::fs`, which blocks the thread the future is polled on, as the files are small.

use std::collections::BTreeMap;

use anyhow::{anyhow, Context};
use futures::channel::mpsc;
use futures::stream::{self, BoxStream};
use futures::StreamExt;
use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use stores_protocol::{
    AssemblyRulesViewItem, BomViewItem, EdaPlacementsViewItem, EdaSubstitutionRuleTransformItem, EdaTool, FieldPattern,
    Part, PartMappingsViewItem, Parts, PartsSource, PartsViewItem, PartsWriteError, Pattern, PlacementsViewItem,
    Records, RecordsKind, RecordsOf, RefDes, Revision, Source, StoresOutput, SubstitutionsViewItem, Table, TableKind,
    TableRecords,
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
            ));
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

/// The data source's records, of the kind, as a table of view items, and their revision, or `None` if there's no file
/// (`STORE-17`).
///
/// Each record is read as the core that uses it reads it, a pattern's regular expression parsed once, here, kept even if
/// it isn't valid, so that it's shown as it is. A record that can't be read, such as one with an unknown EDA tool, is
/// refused, saying why.
pub async fn read_table(kind: TableKind, source: &Source) -> Result<Option<TableRecords>, String> {
    let Source::File(path) = source else {
        return Err(format!("Unsupported source. source: {:?}", source));
    };

    info!("Loading {}. source: '{}'", table_name(kind), source);
    // read once, so the revision is of the contents the table was read from.
    let contents = match std::fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Unable to read the table. source: '{}', cause: '{}'",
                source, error
            ));
        }
    };
    let revision = revision(&contents);

    let table = read_table_of(kind, contents.as_slice())
        .map_err(|error| format!("Unable to read the table. source: '{}', cause: '{:#}'", source, error))?;

    Ok(Some(TableRecords {
        revision,
        table,
    }))
}

/// The records, of the kind, as view items.
pub fn read_table_of(kind: TableKind, contents: &[u8]) -> anyhow::Result<Table> {
    Ok(match kind {
        TableKind::EdaPlacements {
            eda_tool,
        } => Table::EdaPlacements(
            stores::eda_placements::read_eda_placements(eda_tool, contents)?
                .into_iter()
                .map(|placement| EdaPlacementsViewItem {
                    placement,
                })
                .collect(),
        ),
        TableKind::PartMappings => Table::PartMappings(
            stores::csv::read_rows(contents)?
                .iter()
                .map(part_mapping_view_item)
                .collect::<anyhow::Result<_>>()?,
        ),
        TableKind::Substitutions => Table::Substitutions(
            stores::csv::read_rows(contents)?
                .iter()
                .map(substitution_view_item)
                .collect::<anyhow::Result<_>>()?,
        ),
        TableKind::AssemblyRules => Table::AssemblyRules(
            stores::csv::read_rows(contents)?
                .into_iter()
                .map(|row| {
                    let fields = row.into_iter().collect();
                    let rule = stores::csv::AssemblyRuleRecord::from_fields(&fields)?.build_assembly_rule()?;
                    Ok(AssemblyRulesViewItem {
                        rule,
                    })
                })
                .collect::<anyhow::Result<_>>()?,
        ),
        TableKind::Placements => Table::Placements(
            stores::placements::read_all_placements(contents)?
                .into_iter()
                .map(|placement| PlacementsViewItem {
                    placement,
                })
                .collect(),
        ),
        TableKind::Bom => Table::Bom(
            stores::bom::read_jlcpcb_bom(contents)?
                .into_iter()
                .map(|record| BomViewItem {
                    // as the Variant Builder joins them.
                    designators: record
                        .designator
                        .split(',')
                        .map(str::trim)
                        .filter(|ref_des| !ref_des.is_empty())
                        .map(|ref_des| RefDes::from(ref_des.to_string()))
                        .collect(),
                    footprint: record.footprint,
                    jlcpcb_part: record
                        .jlcpcb_part
                        .filter(|part| !part.is_empty()),
                    quantity: record.quantity,
                    comment: record.comment,
                })
                .collect(),
        ),
    })
}

/// The row's EDA tool, from its `Eda` value.
fn eda_tool(row: &BTreeMap<String, String>) -> anyhow::Result<EdaTool> {
    let eda = field(row, "Eda")?;
    stores::csv::csv_eda_tool_value_to_eda_tool(eda).ok_or_else(|| anyhow!("Unknown EDA. value: {:?}", eda))
}

fn field<'a>(row: &'a BTreeMap<String, String>, name: &str) -> anyhow::Result<&'a String> {
    row.get(name)
        .ok_or_else(|| anyhow!("Missing field. field: {:?}", name))
}

/// A part mapping, its EDA tool's fields' patterns found as the Planner's part mapper finds them, by name, in any case.
fn part_mapping_view_item(row: &BTreeMap<String, String>) -> anyhow::Result<PartMappingsViewItem> {
    let eda_tool = eda_tool(row)?;
    let criteria = stores::csv::eda_fields_names(&eda_tool)
        .iter()
        .filter_map(|&field_name| {
            row.iter()
                .find(|(key, _)| key.to_lowercase() == field_name)
                .map(|(_, value)| FieldPattern {
                    field_name: field_name.to_string(),
                    pattern: Pattern::parse(value),
                })
        })
        .collect();

    Ok(PartMappingsViewItem {
        eda_tool,
        criteria,
        part: Part {
            manufacturer: field(row, "Manufacturer")?.clone(),
            mpn: field(row, "Mpn")?.clone(),
        },
    })
}

/// A substitution, with a pattern and a value for each of its EDA tool's fields, as the Variant Builder reads it.
fn substitution_view_item(row: &BTreeMap<String, String>) -> anyhow::Result<SubstitutionsViewItem> {
    let eda_tool = eda_tool(row)?;
    let mut criteria = vec![];
    let mut transforms = vec![];
    for &field_name in stores::csv::eda_fields_names(&eda_tool) {
        let pattern = field(row, &stores::csv::pattern_column_name(field_name))?;
        let value = field(row, &stores::csv::field_column_name(field_name))?;
        criteria.push(FieldPattern {
            field_name: field_name.to_string(),
            pattern: Pattern::parse(pattern),
        });
        transforms.push(EdaSubstitutionRuleTransformItem {
            field_name: field_name.to_string(),
            field_value: value.clone(),
        });
    }

    Ok(SubstitutionsViewItem {
        eda_tool,
        criteria,
        transforms,
        comment: row
            .get("Comment")
            .filter(|comment| !comment.is_empty())
            .cloned(),
    })
}

/// What a table's records are called in the logs, as other records are.
fn table_name(kind: TableKind) -> &'static str {
    match kind {
        TableKind::EdaPlacements {
            ..
        } => "eda placements",
        TableKind::PartMappings => "part mappings",
        TableKind::Substitutions => "eda substitutions",
        TableKind::AssemblyRules => "assembly rules",
        TableKind::Placements => "placements",
        TableKind::Bom => "bom",
    }
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

/// Writes the parts, replacing the file, only if its contents are still those of `revision`, the revision the change
/// was based on, and returns the revision written (`STORE-15`).
///
/// The file's headers keep their order. The check and the write aren't one operation, so a change made by another
/// process between them is lost, though the time between them is short.
pub async fn write_parts(
    source: &PartsSource,
    revision: &Revision,
    parts: &[PartsViewItem],
) -> Result<Revision, PartsWriteError> {
    let Source::File(path) = source else {
        return Err(PartsWriteError::Failed {
            cause: format!("Unsupported source. source: {:?}", source),
        });
    };
    let failed = |cause: String| PartsWriteError::Failed {
        cause: format!("Unable to write the parts. source: '{}', cause: '{}'", source, cause),
    };

    info!("Storing parts. source: '{}'", source);
    let current = match std::fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(PartsWriteError::Changed {
                revision: None,
            });
        }
        Err(error) => return Err(failed(error.to_string())),
    };
    let current_revision = self::revision(&current);
    if current_revision != *revision {
        return Err(PartsWriteError::Changed {
            revision: Some(current_revision),
        });
    }

    let headers = stores::parts::read_headers(current.as_slice()).map_err(|error| failed(format!("{:#}", error)))?;
    let parts: Vec<_> = parts
        .iter()
        .map(|item| (item.part.clone(), item.fields.clone()))
        .collect();
    let mut contents = vec![];
    stores::parts::write_parts(&mut contents, &headers, &parts).map_err(|error| failed(format!("{:#}", error)))?;

    std::fs::write(path, &contents).map_err(|error| failed(error.to_string()))?;

    Ok(self::revision(&contents))
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
