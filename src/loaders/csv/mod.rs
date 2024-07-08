use thiserror::Error;
use crate::eda::diptrace::csv::DipTracePartMappingRecord;
use crate::pnp::part::Part;
use crate::eda::diptrace::criteria::ExactMatchCriteria;
use crate::part_mapper::criteria::PlacementMappingCriteria;
use crate::part_mapper::part_mapping::PartMapping;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct CSVPartMappingRecord {
    eda: String,
    name: String,
    value: String,
    manufacturer: String,
    mpn: String,
}

pub enum PartMappingRecord {
    DipTracePartMapping(DipTracePartMappingRecord),
    //KiCadPartMapping(KiCadPartMappingRecord),
}

#[derive(Error, Debug)]
pub enum CSVPartMappingRecordError {
    #[error("Unknown EDA: '{eda:?}'")]
    UnknownEDA { eda: String }
}

impl TryFrom<CSVPartMappingRecord> for PartMappingRecord {
    type Error = CSVPartMappingRecordError;

    fn try_from(value: CSVPartMappingRecord) -> Result<Self, Self::Error> {
        match value.eda.as_str() {
            "DipTrace" => Ok(PartMappingRecord::DipTracePartMapping(DipTracePartMappingRecord {
                name: value.name.to_string(),
                value: value.value.to_string(),
                manufacturer: value.manufacturer.to_string(),
                mpn: value.mpn.to_string(),
            })),
            _ => Err(CSVPartMappingRecordError::UnknownEDA { eda: value.eda }),
        }
    }
}

impl PartMappingRecord {
    pub fn build_part_mapping<'part>(&self, parts: &'part Vec<Part>) -> Result<PartMapping<'part>, ()> {

        let part_criteria: Part = match self {
            PartMappingRecord::DipTracePartMapping(r) => Ok(Part { manufacturer: r.manufacturer.clone(), mpn: r.mpn.clone() }),
            //_ => Err(()) // TODO - unable to build part criteria
        }?;

        let matched_part_ref = parts.iter().find_map(|part| {
            match part.eq(&part_criteria) {
                true => Some(part),
                false => None,
            }
        });

        let part_ref = match matched_part_ref {
            Some(part) => Ok(part),
            _ => Err(()) // TODO
        }?;

        let criterion = match self {
            PartMappingRecord::DipTracePartMapping(record) => {
                Ok(ExactMatchCriteria::new(record.name.clone(), record.value.clone()))
            }
        }?;


        let criteria: Vec<Box<dyn PlacementMappingCriteria>> = vec![Box::new(criterion)];

        let part_mapping = PartMapping::new(part_ref, criteria);

        Ok(part_mapping)
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct PartRecord {
    manufacturer: String,
    mpn: String,
}

impl PartRecord {
    pub fn build_part(&self) -> Result<Part, ()> {
        Ok(Part {
            manufacturer: self.manufacturer.clone(),
            mpn: self.mpn.clone(),
        })
    }
}