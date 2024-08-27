use std::collections::BTreeMap;
use std::path::PathBuf;
use tracing::trace;
use rust_decimal::Decimal;
use anyhow::Context;
use planning::design::DesignVariant;
use pnp::pcb::PcbSide;
use pnp::part::Part;
use pnp::placement::Placement;

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
            ref_des: self.ref_des.clone(),
            part: Part { manufacturer: self.manufacturer.clone(), mpn: self.mpn.clone() },
            place: self.place,
            pcb_side: PcbSide::from(&self.pcb_side),
            x: self.x,
            y: self.y,
            rotation: self.rotation,
        }
    }
}

pub fn load_placements(placements_path: PathBuf) -> Result<Vec<Placement>, anyhow::Error>{
    let mut csv_reader = csv::ReaderBuilder::new()
        .from_path(placements_path.clone())
        .with_context(|| format!("Error placements. file: {}", placements_path.to_str().unwrap()))?;

    let records = csv_reader.deserialize()
        .inspect(|record| {
            trace!("{:?}", record);
        })
        .filter_map(|record: Result<PlacementRecord, csv::Error> | {
            // TODO report errors
            match record {
                Ok(record) => Some(record.as_placement()),
                _ => None
            }
        })
        .collect();

    Ok(records)
}

pub fn load_all_placements(unique_design_variants: &[DesignVariant], path: &PathBuf) -> anyhow::Result<BTreeMap<DesignVariant, Vec<Placement>>> {
    let mut all_placements: BTreeMap<DesignVariant, Vec<Placement>> = Default::default();

    for design_variant in unique_design_variants {
        let DesignVariant { design_name: design, variant_name: variant } = design_variant;

        let mut placements_path = PathBuf::from(path);
        placements_path.push(format!("{}_{}_placements.csv", design, variant));

        let placements = load_placements(placements_path)?;
        let _ = all_placements.insert(design_variant.clone(), placements);
    }

    Ok(all_placements)
}
