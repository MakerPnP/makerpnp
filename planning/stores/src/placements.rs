use pnp::part::Part;
use pnp::pcb::PcbSide;
use pnp::placement::Placement;
use rust_decimal::Decimal;
use tracing::trace;
use util::source::Source;

/// See `EdaPlacement` for details of co-ordinate system
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlacementRecord {
    pub ref_des: String,
    pub manufacturer: String,
    pub mpn: String,
    pub place: bool,
    pub pcb_side: PlacementRecordPcbSide,
    pub x: Decimal,
    pub y: Decimal,
    pub rotation: Decimal,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub enum PlacementRecordPcbSide {
    Top,
    Bottom,
}

impl From<&PlacementRecordPcbSide> for PcbSide {
    fn from(value: &PlacementRecordPcbSide) -> Self {
        match value {
            PlacementRecordPcbSide::Top => PcbSide::Top,
            PlacementRecordPcbSide::Bottom => PcbSide::Bottom,
        }
    }
}

impl From<&PcbSide> for PlacementRecordPcbSide {
    fn from(value: &PcbSide) -> Self {
        match value {
            PcbSide::Top => PlacementRecordPcbSide::Top,
            PcbSide::Bottom => PlacementRecordPcbSide::Bottom,
        }
    }
}

impl PlacementRecord {
    pub fn as_placement(&self) -> Placement {
        Placement {
            ref_des: self.ref_des.clone().into(),
            part: Part {
                manufacturer: self.manufacturer.clone(),
                mpn: self.mpn.clone(),
            },
            place: self.place,
            pcb_side: PcbSide::from(&self.pcb_side),
            x: self.x,
            y: self.y,
            rotation: self.rotation,
        }
    }
}

pub type PlacementsSource = Source;

/// The placements, from their CSV. A record that can't be read is skipped.
pub fn read_placements(reader: impl std::io::Read) -> Result<Vec<Placement>, anyhow::Error> {
    let mut csv_reader = csv::ReaderBuilder::new().from_reader(reader);

    let records = csv_reader
        .deserialize()
        .inspect(|record| {
            trace!("{:?}", record);
        })
        .filter_map(|record: Result<PlacementRecord, csv::Error>| {
            // TODO report errors
            match record {
                Ok(record) => Some(record.as_placement()),
                _ => None,
            }
        })
        .collect();

    Ok(records)
}

/// Every placement of their CSV, in order, refusing a record that can't be read, so that it's shown as it is
/// (`STORE-17`).
pub fn read_all_placements(reader: impl std::io::Read) -> Result<Vec<Placement>, anyhow::Error> {
    let mut csv_reader = csv::ReaderBuilder::new().from_reader(reader);

    csv_reader
        .deserialize()
        .map(|record: Result<PlacementRecord, csv::Error>| {
            record
                .map(|record| record.as_placement())
                .map_err(anyhow::Error::from)
        })
        .collect()
}
