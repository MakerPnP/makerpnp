use std::path::PathBuf;
use anyhow::{bail, Error};
use tracing::trace;
use crate::assembly::rules::AssemblyRule;
use crate::loaders::csv::AssemblyRuleRecord;

#[tracing::instrument]
pub fn load(assembly_rule_source: &String) -> Result<Vec<AssemblyRule>, Error>  {
    let assembly_rule_path_buf = PathBuf::from(assembly_rule_source);
    let assembly_rule_path = assembly_rule_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(assembly_rule_path)?;

    let mut assembly_rules: Vec<AssemblyRule> = vec![];

    for result in csv_reader.deserialize() {
        let record: AssemblyRuleRecord = result?;
        trace!("{:?}", record);

        if let Ok(assembly_rule) = record.build_assembly_rule() {
            assembly_rules.push(assembly_rule);
        } else {
            bail!("todo")
        }
    }
    Ok(assembly_rules)
}