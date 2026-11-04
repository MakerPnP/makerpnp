use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Error};
use assembly::rules::AssemblyRule;
use tracing::Level;
use tracing::{info, trace};
use util::source::Source;

use crate::csv::AssemblyRuleRecord;

pub type AssemblyRuleSource = Source;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load(source: &AssemblyRuleSource) -> Result<Vec<AssemblyRule>, Error> {
    info!("Loading assembly rules. source: {}", source);

    let path = source
        .path()
        .map_err(|error| anyhow!("Unsupported source type. cause: {:?}", error))?;

    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(path.clone())
        .with_context(|| format!("Error reading assembly rules. file: {}", path.display()))?;

    let mut assembly_rules: Vec<AssemblyRule> = vec![];

    for result in csv_reader.deserialize() {
        let record: AssemblyRuleRecord = result.with_context(|| "Deserializing assembly rule record".to_string())?;

        trace!("{:?}", record);

        let assembly_rule = record
            .build_assembly_rule()
            .with_context(|| format!("Building assembly rule from record. record: {:?}", record))?;

        assembly_rules.push(assembly_rule);
    }
    Ok(assembly_rules)
}

/// The assembly rules, made from each one's fields, see [`crate::csv::read_rows`].
pub fn build(rows: &[BTreeMap<String, String>]) -> Result<Vec<AssemblyRule>, Error> {
    rows.iter()
        .map(|row| {
            let fields = row.clone().into_iter().collect();
            let record = AssemblyRuleRecord::from_fields(&fields)
                .with_context(|| format!("Building assembly rule from record. record: {:?}", row))?;
            trace!("{:?}", record);
            record
                .build_assembly_rule()
                .with_context(|| format!("Building assembly rule from record. record: {:?}", record))
        })
        .collect()
}
