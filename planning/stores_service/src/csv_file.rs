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
    quoted_list, AddColumnsError, AssemblyRulesViewItem, BomViewItem, ColumnPosition, ColumnRef, ColumnValue,
    EdaPlacementsViewItem, EdaSubstitutionRuleTransformItem, EdaTool, FieldPattern, FieldValue, Part,
    PartMappingsViewItem, Parts, PartsSource, PartsViewItem, PartsWriteError, Pattern, PlacementsViewItem, Records,
    RecordsKind, RecordsOf, RefDes, Revision, RowOrdering, RowRef, RowsChange, Source, StoresOutput,
    SubstitutionsViewItem, Table, TableColumn, TableKind, TableRecords,
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

    let mut records = TableRecords::read_only(revision, table);
    // `STORE-19`, part mappings can be changed: a row's key is its place in the file, as it was read, a column's
    // reference its header, and the rows are ordered by their place.
    if kind == TableKind::PartMappings {
        let headers = stores::csv::read_headers(contents.as_slice())
            .map_err(|error| format!("Unable to read the table. source: '{}', cause: '{:#}'", source, error))?;
        // `STORE-17`, a pattern's column for each EDA tool's field the file has a column for, even with no rows, so the
        // core knows which an EDA tool's rows still need (`STORE-21`).
        records.columns = TableColumn::part_mappings(
            EdaTool::all_field_names()
                .into_iter()
                .filter(|field_name| {
                    headers
                        .iter()
                        .any(|header| header.to_lowercase() == *field_name)
                }),
        );
        for column in records.columns.iter_mut() {
            column.reference = part_mapping_reference(&column.value, &headers);
        }
        records.ordering = RowOrdering::Position;
    }
    // `STORE-19`, substitutions likewise, with a pattern's and a value's column for each EDA tool's field the file has
    // either for, and a comment's if it has one (`STORE-17`, `STORE-21`).
    if kind == TableKind::Substitutions {
        let headers = stores::csv::read_headers(contents.as_slice())
            .map_err(|error| format!("Unable to read the table. source: '{}', cause: '{:#}'", source, error))?;
        let has = |name: String| headers.contains(&name);
        let field_names = EdaTool::all_field_names();
        let fields = |name: fn(&str) -> String| -> Vec<&str> {
            field_names
                .iter()
                .copied()
                .filter(|field_name| has(name(field_name)))
                .collect()
        };
        records.columns = TableColumn::substitutions(
            &fields(TableColumn::substitutions_pattern_name),
            &fields(TableColumn::substitutions_value_name),
            has("Comment".to_string()),
        );
        for column in records.columns.iter_mut() {
            column.reference = substitution_reference(&column.value, &headers);
        }
        records.ordering = RowOrdering::Position;
    }
    // `STORE-19`, assembly rules likewise, their columns' references their headers, as they're written, as the Variant
    // Builder reads them, a file without one refused, saying which, even with no rows (`STORE-17`).
    if kind == TableKind::AssemblyRules {
        let headers = stores::csv::read_headers(contents.as_slice())
            .map_err(|error| format!("Unable to read the table. source: '{}', cause: '{:#}'", source, error))?;
        for column in records.columns.iter_mut() {
            let name = match column.value {
                ColumnValue::RefDes => "RefDes",
                ColumnValue::Manufacturer => "Manufacturer",
                ColumnValue::Mpn => "Mpn",
                _ => continue,
            };
            if !headers
                .iter()
                .any(|header| header == name)
            {
                return Err(format!(
                    "Unable to read the table. source: '{}', cause: 'Missing field. field: \"{}\"'",
                    source, name
                ));
            }
            column.reference = Some(ColumnRef(name.to_string()));
        }
        records.ordering = RowOrdering::Position;
    }

    Ok(Some(records))
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

/// A part mapping, with a pattern for each field of any EDA tool the file has a column for, not only its own tool's, in
/// the EDA tools' order, found as the Planner's part mapper finds them, by name, in any case, so every value is shown
/// whichever tool the row has.
fn part_mapping_view_item(row: &BTreeMap<String, String>) -> anyhow::Result<PartMappingsViewItem> {
    let eda_tool = eda_tool(row)?;
    let criteria = EdaTool::all_field_names()
        .into_iter()
        .filter_map(|field_name| {
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

/// The header a part mapping's column's values are in, as [`part_mapping_view_item`] finds them, if the file has it.
fn part_mapping_reference(value: &ColumnValue, headers: &[String]) -> Option<ColumnRef> {
    let found = |matches: &dyn Fn(&String) -> bool| {
        headers
            .iter()
            .find(|header| matches(header))
            .map(|header| ColumnRef(header.clone()))
    };
    match value {
        ColumnValue::Eda => found(&|header| header == "Eda"),
        ColumnValue::Manufacturer => found(&|header| header == "Manufacturer"),
        ColumnValue::Mpn => found(&|header| header == "Mpn"),
        ColumnValue::Pattern(field_name) => found(&|header| header.to_lowercase() == *field_name),
        _ => None,
    }
}

/// A substitution, with a pattern for each field of any EDA tool the file has a pattern's column for, and a value for
/// each it has a value's column for, not only its own tool's, in the EDA tools' order, found by their names as the
/// Variant Builder names them, so every value is shown whichever tool the row has. One whose tool's field has no column
/// is read without it, so it can be shown, and given the column, though the Variant Builder refuses it (`STORE-17`).
fn substitution_view_item(row: &BTreeMap<String, String>) -> anyhow::Result<SubstitutionsViewItem> {
    let eda_tool = eda_tool(row)?;
    let field_names = EdaTool::all_field_names();
    let criteria = field_names
        .iter()
        .filter_map(|&field_name| {
            row.get(&TableColumn::substitutions_pattern_name(field_name))
                .map(|pattern| FieldPattern {
                    field_name: field_name.to_string(),
                    pattern: Pattern::parse(pattern),
                })
        })
        .collect();
    let transforms = field_names
        .iter()
        .filter_map(|&field_name| {
            row.get(&TableColumn::substitutions_value_name(field_name))
                .map(|value| EdaSubstitutionRuleTransformItem {
                    field_name: field_name.to_string(),
                    field_value: value.clone(),
                })
        })
        .collect();

    Ok(SubstitutionsViewItem {
        eda_tool,
        criteria,
        transforms,
        comment: row.get("Comment").cloned(),
    })
}

/// The header a substitution's column's values are in, as [`substitution_view_item`] finds them, if the file has it.
fn substitution_reference(value: &ColumnValue, headers: &[String]) -> Option<ColumnRef> {
    let name = match value {
        ColumnValue::Eda => "Eda".to_string(),
        ColumnValue::Pattern(field_name) => TableColumn::substitutions_pattern_name(field_name),
        ColumnValue::Substitute(field_name) => TableColumn::substitutions_value_name(field_name),
        ColumnValue::Comment => "Comment".to_string(),
        _ => return None,
    };
    headers
        .contains(&name)
        .then(|| ColumnRef(name))
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

/// Changes the file's rows, by their keys, their places in the file when they were read, and their columns' headers,
/// only if each value is as it was, and returns the file's revision once it's written (`STORE-19`).
///
/// Only the columns named are changed, so a column the table doesn't show is left as it was. A row whose values aren't
/// as they were, or that isn't there, has changed since it was read, as when a row before it was added or removed, so
/// the change is refused, saying the file's revision now.
pub async fn change_rows(kind: TableKind, source: &Source, change: &RowsChange) -> Result<Revision, PartsWriteError> {
    let Source::File(path) = source else {
        return Err(PartsWriteError::Failed {
            cause: format!("Unsupported source. source: {:?}", source),
        });
    };
    let failed = |cause: String| PartsWriteError::Failed {
        cause: format!("Unable to change the table. source: '{}', cause: '{}'", source, cause),
    };
    if !matches!(
        kind,
        TableKind::PartMappings | TableKind::Substitutions | TableKind::AssemblyRules
    ) {
        return Err(failed(format!("Unsupported table. kind: {:?}", kind)));
    }

    info!("Changing {}. source: '{}'", table_name(kind), source);
    let current = match std::fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(PartsWriteError::Changed {
                revision: None,
            });
        }
        Err(error) => return Err(failed(error.to_string())),
    };
    let changed = || PartsWriteError::Changed {
        revision: Some(self::revision(&current)),
    };

    let (headers, mut records) =
        stores::csv::read_records(current.as_slice()).map_err(|error| failed(format!("{:#}", error)))?;
    let column = |reference: &ColumnRef| {
        headers
            .iter()
            .position(|header| *header == reference.0)
            .ok_or_else(|| failed(format!("The column doesn't exist. column: '{}'", reference)))
    };
    // the row's place, if it's there, and its values are those expected, so it's the row the change was made to.
    let row = |records: &[Vec<String>], row: &RowRef| -> Result<usize, PartsWriteError> {
        let index = row
            .key
            .0
            .parse::<usize>()
            .map_err(|_| failed(format!("Unknown row. key: '{}'", row.key)))?;
        let record = records.get(index).ok_or_else(changed)?;
        for expected in row.expected.iter() {
            if record[column(&expected.column)?] != expected.value {
                return Err(changed());
            }
        }
        Ok(index)
    };

    match change {
        RowsChange::Update {
            key,
            changes,
        } => {
            let expected = changes
                .iter()
                .map(|change| FieldValue {
                    column: change.column.clone(),
                    value: change.old.clone(),
                })
                .collect();
            let index = row(&records, &RowRef {
                key: key.clone(),
                expected,
            })?;
            for change in changes.iter() {
                let column = column(&change.column)?;
                records[index][column] = change.new.clone();
            }
        }
        RowsChange::Add {
            values,
        } => {
            let mut record = vec![String::new(); headers.len()];
            for value in values.iter() {
                record[column(&value.column)?] = value.value.clone();
            }
            records.push(record);
        }
        RowsChange::Remove {
            row: removed,
        } => {
            let index = row(&records, removed)?;
            records.remove(index);
        }
        RowsChange::Move {
            row: moved,
            before,
        } => {
            let from = row(&records, moved)?;
            let before = before
                .as_ref()
                .map(|before| row(&records, before))
                .transpose()?;
            let record = records.remove(from);
            let to = match before {
                // the row before which it's moved is a place nearer, as the row moved was before it.
                Some(before) if before > from => before - 1,
                Some(before) => before,
                None => records.len(),
            };
            records.insert(to, record);
        }
    }

    let mut contents = vec![];
    stores::csv::write_records(&mut contents, &headers, &records).map_err(|error| failed(format!("{:#}", error)))?;

    std::fs::write(path, &contents).map_err(|error| failed(error.to_string()))?;

    Ok(self::revision(&contents))
}

/// Adds the columns to the file, each header inserted where it's to go, in the order given, with an empty value in each
/// row, every other column, value and row left as they were, and returns the file's revision once it's written
/// (`STORE-21`).
///
/// A column is already there if a header has its name, in any case, as the part mapper finds a field's column. The
/// column they're to go before or after is named by its header, as it's written, as a column's reference is.
pub async fn add_columns(
    kind: TableKind,
    source: &Source,
    columns: &[String],
    position: &ColumnPosition,
) -> Result<Revision, AddColumnsError> {
    let Source::File(path) = source else {
        return Err(AddColumnsError::Failed {
            cause: format!("Unsupported source. source: {:?}", source),
        });
    };
    let failed = |cause: String| AddColumnsError::Failed {
        cause: format!("Unable to add the columns. source: '{}', cause: '{}'", source, cause),
    };
    if !matches!(
        kind,
        TableKind::PartMappings | TableKind::Substitutions | TableKind::AssemblyRules
    ) {
        return Err(failed(format!("Unsupported table. kind: {:?}", kind)));
    }
    // the core refuses them, as assembly rules have no EDA tool.
    if kind == TableKind::AssemblyRules && *position == ColumnPosition::EdaToolColumns {
        return Err(failed("The table has no EDA tool's columns.".to_string()));
    }

    info!(
        "Adding columns. source: '{}', columns: {}",
        source,
        quoted_list(columns)
    );
    let current = match std::fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(AddColumnsError::SourceNotFound),
        Err(error) => return Err(failed(error.to_string())),
    };
    let (mut headers, mut records) =
        stores::csv::read_records(current.as_slice()).map_err(|error| failed(format!("{:#}", error)))?;

    for (index, column) in columns.iter().enumerate() {
        let is_named = |header: &String| header.to_lowercase() == column.to_lowercase();
        if headers.iter().any(is_named) || columns[..index].iter().any(is_named) {
            return Err(AddColumnsError::ColumnExists {
                column: column.clone(),
            });
        }
    }
    let place = |column: &String| {
        headers
            .iter()
            .position(|header| header == column)
            .ok_or_else(|| AddColumnsError::ColumnNotFound {
                column: column.clone(),
            })
    };
    // each column's place, the header it's inserted before, as the file has them.
    let places: Vec<usize> = match position {
        ColumnPosition::EdaToolColumns if kind == TableKind::Substitutions => {
            substitutions_eda_tool_columns_places(&headers, columns)
        }
        position => {
            let at = match position {
                ColumnPosition::First => 0,
                ColumnPosition::Last => headers.len(),
                ColumnPosition::Before(column) => place(column)?,
                ColumnPosition::After(column) => place(column)? + 1,
                ColumnPosition::EdaToolColumns => part_mappings_eda_tool_columns_place(&headers),
            };
            vec![at; columns.len()]
        }
    };

    // the last place first, so the places before it are as they were, those at the same place in the order given.
    let mut inserts: Vec<(usize, &String)> = places
        .into_iter()
        .zip(columns.iter())
        .collect();
    inserts.sort_by_key(|(place, _)| *place);
    for (place, column) in inserts.into_iter().rev() {
        headers.insert(place, column.clone());
        for record in records.iter_mut() {
            record.insert(place, String::new());
        }
    }

    let mut contents = vec![];
    stores::csv::write_records(&mut contents, &headers, &records).map_err(|error| failed(format!("{:#}", error)))?;

    std::fs::write(path, &contents).map_err(|error| failed(error.to_string()))?;

    Ok(self::revision(&contents))
}

/// Where an EDA tool's columns go in a part mappings file (`STORE-21`): after its last pattern's column, if it has a
/// manufacturer's or an MPN's column, and every pattern's column is before them, so before them if it has no pattern's
/// column, or else after its last column.
fn part_mappings_eda_tool_columns_place(headers: &[String]) -> usize {
    let field_names = EdaTool::all_field_names();
    let places = |matches: &dyn Fn(&String) -> bool| -> Vec<usize> {
        headers
            .iter()
            .enumerate()
            .filter(|(_, header)| matches(header))
            .map(|(place, _)| place)
            .collect()
    };
    // found as `part_mapping_reference` finds them.
    let patterns = places(&|header| field_names.contains(&header.to_lowercase().as_str()));
    let part = places(&|header| header == "Manufacturer" || header == "Mpn");
    match (part.first(), patterns.last()) {
        (Some(&part), Some(&last)) if last < part => last + 1,
        (Some(&part), None) => part,
        _ => headers.len(),
    }
}

/// Where each of an EDA tool's columns goes in a substitutions file (`STORE-21`), the header it's inserted before,
/// whose patterns' columns, and values' columns, are each a group: a pattern's after the last pattern's column, and a
/// value's after the last value's column; if it has no pattern's column, before the first value's column, and if it
/// has no value's column, after the last pattern's column; and if it has neither, before its comment's column, or, if
/// it has none, after its last column. A column is a pattern's or a value's as the Variant Builder names them.
fn substitutions_eda_tool_columns_places(headers: &[String], columns: &[String]) -> Vec<usize> {
    let field_names = EdaTool::all_field_names();
    let is_pattern = |header: &String| {
        field_names
            .iter()
            .any(|field_name| *header == TableColumn::substitutions_pattern_name(field_name))
    };
    let is_value = |header: &String| {
        field_names
            .iter()
            .any(|field_name| *header == TableColumn::substitutions_value_name(field_name))
    };
    let places = |matches: &dyn Fn(&String) -> bool| -> Vec<usize> {
        headers
            .iter()
            .enumerate()
            .filter(|(_, header)| matches(header))
            .map(|(place, _)| place)
            .collect()
    };
    let patterns = places(&is_pattern);
    let values = places(&is_value);
    let neither = headers
        .iter()
        .position(|header| header == "Comment")
        .unwrap_or(headers.len());
    let after_patterns = patterns.last().map(|last| last + 1);
    let after_values = values.last().map(|last| last + 1);

    let pattern_place = after_patterns
        .or(values.first().copied())
        .unwrap_or(neither);
    let value_place = after_values
        .or(after_patterns)
        .unwrap_or(neither);
    columns
        .iter()
        .map(|column| match is_pattern(column) {
            true => pattern_place,
            false => value_place,
        })
        .collect()
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
