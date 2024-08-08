use std::ops::{Add, Sub};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use thiserror::Error;
use crate::eda::placement::{EdaPlacement, EdaPlacementField};
use crate::planning::PcbSide;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct DiptracePlacementRecord {
    ref_des: String,
    name: String,
    value: String,
    side: DipTracePcbSide,
    x: Decimal,
    y: Decimal,
    /// Positive values indicate anti-clockwise rotation
    /// Range is 0 - < 360
    /// Rounding occurs on the 3rd decimal, e.g. 359.991 rounds to 359.99, 359.995 rounds to 360, then gets converted to 0.
    rotation: Decimal,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all(deserialize = "PascalCase"))]
enum DipTracePcbSide {
    Top,
    Bottom,
}

impl From<&DipTracePcbSide> for PcbSide {
    fn from(value: &DipTracePcbSide) -> Self {
        match value {
            DipTracePcbSide::Top => PcbSide::Top,
            DipTracePcbSide::Bottom => PcbSide::Bottom,
        }
    }
}

#[derive(Error, Debug)]
pub enum DiptracePlacementRecordError {
    #[error("Unknown")]
    Unknown
}

impl DiptracePlacementRecord {
    pub fn build_eda_placement(&self) -> Result<EdaPlacement, DiptracePlacementRecordError> {
        Ok(EdaPlacement {
            ref_des: self.ref_des.to_string(),
            place: true,
            fields: vec![
                EdaPlacementField { name: "name".to_string(), value: self.name.to_string() },
                EdaPlacementField { name: "value".to_string(), value: self.value.to_string() },
            ],
            pcb_side: PcbSide::from(&self.side),
            x: self.x,
            y: self.y,
            rotation: DipTraceRotationConverter::convert(self.rotation),
        })

        // _ => Err(DiptracePlacementRecordError::Unknown)
    }
}

struct DipTraceRotationConverter {}
impl DipTraceRotationConverter {
    pub fn convert(mut input: Decimal) -> Decimal {
        while input >= dec!(360) {
            input = input.sub(dec!(360));
        }
        while input < dec!(0) {
            input = input.add( dec!(360));
        }
        if input > dec!(180) {
            input = input.sub(dec!(360));
        }
        input
    }
}

#[cfg(test)]
mod rotation_conversion_tests {

    use rstest::rstest;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use crate::eda::diptrace::csv::DipTraceRotationConverter;

    #[rstest]
    #[case(dec!(0), dec!(0))]
    #[case(dec!(180), dec!(180))]
    #[case(dec!(-180), dec!(180))]
    #[case(dec!(360), dec!(0))]
    #[case(dec!(185), dec!(-175))]
    #[case(dec!(-185), dec!(175))]
    fn diptrace_to_eda_placement(#[case] value: Decimal, #[case] expected_value: Decimal) {
        assert_eq!(DipTraceRotationConverter::convert(value), expected_value);
    }
}
