use std::path::PathBuf;
use anyhow::{bail, Error};
use tracing::trace;
use crate::eda::eda_substitution::EdaSubstitutionRule;
use crate::loaders::csv::{CSVSubstitutionRecord, SubstitutionRecord};

#[tracing::instrument]
pub fn load_eda_substitutions(substitutions_source: &String) -> Result<Vec<EdaSubstitutionRule>, Error> {
    let substitutions_path_buf = PathBuf::from(substitutions_source);
    let substitutions_path = substitutions_path_buf.as_path();
    let mut csv_reader = csv::ReaderBuilder::new().from_path(substitutions_path)?;

    let mut eda_substitutions: Vec<EdaSubstitutionRule> = vec![];

    for result in csv_reader.deserialize() {
        let record: CSVSubstitutionRecord = result?;
        trace!("{:?}", record);

        let enum_record = SubstitutionRecord::try_from(record)?;

        if let Ok(eda_substitution) = enum_record.build_eda_substitution() {
            eda_substitutions.push(eda_substitution);
        } else {
            bail!("todo")
        }
    }
    Ok(eda_substitutions)
}