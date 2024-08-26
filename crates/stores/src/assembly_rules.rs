use tracing::Level;
use std::path::PathBuf;
use anyhow::{Context, Error};
use tracing::trace;
use assembly::rules::AssemblyRule;
use crate::csv::AssemblyRuleRecord;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load(assembly_rule_source: &String) -> Result<Vec<AssemblyRule>, Error>  {
    let assembly_rule_path_buf = PathBuf::from(assembly_rule_source);
    let assembly_rule_path = assembly_rule_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(assembly_rule_path)
        .with_context(|| format!("Error reading assembly rules. file: {}", assembly_rule_path.to_str().unwrap()))?;

    let mut assembly_rules: Vec<AssemblyRule> = vec![];

    for result in csv_reader.deserialize() {
        let record: AssemblyRuleRecord = result
            .with_context(|| "Deserializing assembly rule record".to_string())?;

        trace!("{:?}", record);

        let assembly_rule = record.build_assembly_rule()
            .with_context(|| format!("Building assembly rule from record. record: {:?}", record))?;

        assembly_rules.push(assembly_rule);
    }
    Ok(assembly_rules)
}