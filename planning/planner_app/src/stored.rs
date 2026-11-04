//! The records of the data sources an event uses, such as a phase's load-out, read by the stores service for it
//! (`STORE-11`).
//!
//! An event asks for the records it needs as it's processed. If they haven't been read, it fails with
//! [`AppError::NeedsRecords`], what it changed is undone, and the stores service reads them, then the event is processed
//! again, with them, see [`crate::Event::WithRecords`]. So an event's code reads as it did when the core read the files
//! itself, and an event that needs records chosen by what it changes, such as the placements of a variant just
//! assigned, reads them too.

use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};

use stores_protocol::{LoadOutItem, Package, Placement, Records, RecordsKind, RecordsOf, Source};

use crate::AppError;

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredRecords {
    records: BTreeMap<(RecordsKind, Source), Result<Option<Records>, String>>,
}

/// Says which records were read, not what they are, as the event is logged.
impl Debug for StoredRecords {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(
                self.records
                    .iter()
                    .map(|((kind, source), result)| (format!("{:?} '{}'", kind, source), result)),
            )
            .finish()
    }
}

impl StoredRecords {
    pub(crate) fn insert(&mut self, kind: RecordsKind, source: Source, result: Result<Option<Records>, String>) {
        self.records
            .insert((kind, source), result);
    }

    /// Requires each of the records to have been read, asking for all of those that haven't at once.
    pub(crate) fn require(&self, needed: &[(RecordsKind, Source)]) -> Result<(), AppError> {
        let missing = needed
            .iter()
            .filter(|key| !self.records.contains_key(key))
            .cloned()
            .collect::<Vec<_>>();
        match missing.is_empty() {
            true => Ok(()),
            false => Err(AppError::NeedsRecords(missing)),
        }
    }

    /// The records, or `None` if there's no data source, or why they couldn't be read.
    fn records(&self, kind: RecordsKind, source: &Source) -> Result<Option<&RecordsOf>, AppError> {
        match self
            .records
            .get(&(kind, source.clone()))
        {
            None => Err(AppError::NeedsRecords(vec![(kind, source.clone())])),
            Some(Err(cause)) => Err(AppError::StoresError(cause.clone())),
            Some(Ok(records)) => Ok(records
                .as_ref()
                .map(|records| &records.records)),
        }
    }

    /// The load-out's items, or `None` if there's no load-out.
    pub(crate) fn load_out(&self, source: &Source) -> Result<Option<Vec<LoadOutItem>>, AppError> {
        match self.records(RecordsKind::LoadOut, source)? {
            Some(RecordsOf::LoadOut(items)) => Ok(Some(items.clone())),
            Some(records) => Err(Self::unexpected(records)),
            None => Ok(None),
        }
    }

    /// The load-out's items, which must exist.
    pub(crate) fn existing_load_out(&self, source: &Source) -> Result<Vec<LoadOutItem>, AppError> {
        self.load_out(source)?
            .ok_or_else(|| Self::not_found(source))
    }

    pub(crate) fn packages(&self, source: &Source) -> Result<&[Package], AppError> {
        match self.records(RecordsKind::Packages, source)? {
            Some(RecordsOf::Packages(packages)) => Ok(packages),
            Some(records) => Err(Self::unexpected(records)),
            None => Err(Self::not_found(source)),
        }
    }

    /// Each package mapping's fields, by name.
    pub(crate) fn package_mappings(&self, source: &Source) -> Result<&[BTreeMap<String, String>], AppError> {
        match self.records(RecordsKind::PackageMappings, source)? {
            Some(RecordsOf::PackageMappings(rows)) => Ok(rows),
            Some(records) => Err(Self::unexpected(records)),
            None => Err(Self::not_found(source)),
        }
    }

    pub(crate) fn placements(&self, source: &Source) -> Result<&[Placement], AppError> {
        match self.records(RecordsKind::Placements, source)? {
            Some(RecordsOf::Placements(placements)) => Ok(placements),
            Some(records) => Err(Self::unexpected(records)),
            None => Err(Self::not_found(source)),
        }
    }

    fn not_found(source: &Source) -> AppError {
        AppError::StoresError(format!("Data source not found. source: '{}'", source))
    }

    fn unexpected(records: &RecordsOf) -> AppError {
        AppError::StoresError(format!("Unexpected records. kind: {:?}", records.kind()))
    }
}
