use tracing::Level;
use std::path::PathBuf;
use anyhow::{bail, Error};
use tracing::trace;
use crate::eda::eda_substitution::EdaSubstitutionRule;
use crate::loaders::csv::SubstitutionRecord;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_eda_substitutions(substitutions_source: &String) -> Result<Vec<EdaSubstitutionRule>, Error> {
    let substitutions_path_buf = PathBuf::from(substitutions_source);
    let substitutions_path = substitutions_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(substitutions_path)?;

    let mut eda_substitutions: Vec<EdaSubstitutionRule> = vec![];

    for result in csv_reader.deserialize() {
        let record: SubstitutionRecord = result?;
        trace!("{:?}", record);

        if let Ok(eda_substitution) = record.build_eda_substitution() {
            eda_substitutions.push(eda_substitution);
        } else {
            bail!("todo")
        }
    }
    Ok(eda_substitutions)
}