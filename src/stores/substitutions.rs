use tracing::Level;
use std::path::PathBuf;
use anyhow::{Context, Error};
use tracing::trace;
use crate::eda::substitution::EdaSubstitutionRule;
use crate::stores::csv::SubstitutionRecord;

#[tracing::instrument(level = Level::DEBUG)]
pub fn load_eda_substitutions(substitutions_source: &String) -> Result<Vec<EdaSubstitutionRule>, Error> {
    let substitutions_path_buf = PathBuf::from(substitutions_source);
    let substitutions_path = substitutions_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(substitutions_path)
        .with_context(|| format!("Error reading substitutions. file: {}", substitutions_path.to_str().unwrap()))?;


    let mut eda_substitutions: Vec<EdaSubstitutionRule> = vec![];

    for result in csv_reader.deserialize() {
        let record: SubstitutionRecord = result
            .with_context(|| "Deserializing substitution record".to_string())?;

        trace!("{:?}", record);

        let eda_substitution = record.build_eda_substitution()
            .with_context(|| format!("Building substitution from record. record: {:?}", record))?;

        eda_substitutions.push(eda_substitution);

    }
    Ok(eda_substitutions)
}