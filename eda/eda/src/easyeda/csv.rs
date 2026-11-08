use std::ops::{Add, Sub};

use pnp::pcb::PcbSide;
use regex::Regex;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use thiserror::Error;

use crate::placement::EdaPlacement;
use crate::EdaTool;

#[derive(Error, Debug)]
pub enum EasyEdaPlacementRecordError {
    #[error("Unknown")]
    Unknown,
    #[error("Unit parse error, cause: {0}")]
    UnitParseError(EasyEdaUnitParserError),
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct EasyEdaPlacementRecord {
    #[serde(rename(deserialize = "Designator"))]
    ref_des: String,
    device: String,
    value: String,
    #[serde(rename(deserialize = "Layer"))]
    side: EasyEdaPcbSide,
    #[serde(rename(deserialize = "Mid X"))]
    x: String,
    #[serde(rename(deserialize = "Mid Y"))]
    y: String,
    /// Positive values indicate anti-clockwise rotation.
    /// Range is >=0 to 359.999
    /// Rounding occurs on the 4th decimal, e.g. 359.9994 rounds to 359.999 and 359.995 rounds to 360, then gets converted to 0.
    rotation: Decimal,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all(deserialize = "lowercase"))]
enum EasyEdaPcbSide {
    #[serde(rename(deserialize = "T"))]
    Top,
    #[serde(rename(deserialize = "B"))]
    Bottom,
}

impl From<&EasyEdaPcbSide> for PcbSide {
    fn from(value: &EasyEdaPcbSide) -> Self {
        match value {
            EasyEdaPcbSide::Top => PcbSide::Top,
            EasyEdaPcbSide::Bottom => PcbSide::Bottom,
        }
    }
}

impl EasyEdaPlacementRecord {
    pub fn build_eda_placement(&self) -> Result<EdaPlacement, EasyEdaPlacementRecordError> {
        let x =
            EasyEdaUnitParser::parse(&self.x).map_err(|cause| EasyEdaPlacementRecordError::UnitParseError(cause))?;
        let y =
            EasyEdaUnitParser::parse(&self.y).map_err(|cause| EasyEdaPlacementRecordError::UnitParseError(cause))?;

        Ok(EdaPlacement {
            ref_des: self.ref_des.to_string(),
            place: true,
            fields: EdaTool::EasyEda.placement_fields(&[&self.device, &self.value]),
            pcb_side: PcbSide::from(&self.side),
            x,
            y,
            rotation: EasyEdaRotationConverter::convert(self.rotation),
        })

        // _ => Err(EasyEdaPlacementRecordError::Unknown)
    }
}

struct EasyEdaRotationConverter {}
impl EasyEdaRotationConverter {
    pub fn convert(mut input: Decimal) -> Decimal {
        while input >= dec!(360) {
            input = input.sub(dec!(360));
        }
        while input < dec!(0) {
            input = input.add(dec!(360));
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

    use crate::easyeda::csv::EasyEdaRotationConverter;

    #[rstest]
    #[case(dec!(0), dec!(0))]
    #[case(dec!(180), dec!(180))]
    #[case(dec!(-180), dec!(180))]
    #[case(dec!(360), dec!(0))]
    #[case(dec!(185), dec!(-175))]
    #[case(dec!(-185), dec!(175))]
    #[case(dec!(0.001), dec!(0.001))]
    #[case(dec!(359.999), dec!(-0.001))]
    fn easyeda_to_eda_placement(#[case] value: Decimal, #[case] expected_value: Decimal) {
        assert_eq!(EasyEdaRotationConverter::convert(value), expected_value);
    }
}

struct EasyEdaUnitParser {}

impl EasyEdaUnitParser {
    /// Extract the decimal value from the input.
    ///
    /// The format is '<decimal-value><unit>', e.g. '359.999mm'
    ///
    /// Currently unit is ignored, users should export pick-and-place files using 'mm' in the EasyEDA UI.
    pub fn parse(input: &String) -> Result<Decimal, EasyEdaUnitParserError> {
        let pattern = Regex::new(r#"^(?<value>[-]?(\d+)+(\.(\d+))?){1}.*"#).unwrap();

        let maybe_value = pattern
            .captures(input)
            .map(|captures| captures.name("value").unwrap().as_str());

        match maybe_value {
            None => Err(EasyEdaUnitParserError::InvalidUnit(input.clone())),
            Some(value) => Ok(Decimal::try_from(value).unwrap()),
        }
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum EasyEdaUnitParserError {
    #[error("Invalid unit. value: {0}")]
    InvalidUnit(String),
}

#[cfg(test)]
mod unit_parser_tests {
    use rstest::rstest;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use crate::easyeda::csv::{EasyEdaUnitParser, EasyEdaUnitParserError};

    #[rstest]
    #[case("3", Ok(dec!(3)))]
    #[case("3mm", Ok(dec!(3)))]
    #[case("0.3mm", Ok(dec!(0.3)))]
    #[case("-3mm", Ok(dec!(-3)))]
    #[case("-.999mm", Err(EasyEdaUnitParserError::InvalidUnit("-.999mm".to_string())))]
    #[case("bananas", Err(EasyEdaUnitParserError::InvalidUnit("bananas".to_string())))]
    #[case("3bananas", Ok(dec!(3)))]
    fn parse(#[case] value: &str, #[case] expected_value: Result<Decimal, EasyEdaUnitParserError>) {
        assert_eq!(EasyEdaUnitParser::parse(&value.to_string()), expected_value);
    }
}
