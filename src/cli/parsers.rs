use std::ffi::{OsStr, OsString};
use clap::builder::TypedValueParser;
use clap::{Arg, Command, Error, value_parser};
use clap::error::ErrorKind;
use crate::cli::args::{PlacementSortingModeArg, SortOrderArg};
use crate::planning::PlacementSortingItem;

#[derive(Clone, Default)]
pub struct PlacementSortingItemParser {}

impl TypedValueParser for PlacementSortingItemParser {
    type Value = PlacementSortingItem;

    /// Parses a value in the format '<MODE>:<SORT_ORDER>' with values in SCREAMING_SNAKE_CASE, e.g. 'FEEDER_REFERENCE:ASC'
    fn parse_ref(&self, cmd: &Command, _arg: Option<&Arg>, value: &OsStr) -> Result<Self::Value, Error> {

        let chunks_str = match value.to_str() {
            Some(str) => Ok(str),
            // TODO create a test for this edge case, how to invoke this code path, is the message helpful to the user, how is it displayed by clap?
            None => Err(Error::raw(ErrorKind::InvalidValue, "Invalid argument encoding")),
        }?;

        let mut chunks: Vec<_> = chunks_str.split(':').collect();
        if chunks.len() != 2 {
            return Err(Error::raw(ErrorKind::InvalidValue, format!("Invalid argument. Required format: '<MODE>:<SORT_ORDER>', found: '{}'", chunks_str)))
        }

        let sort_order_str = chunks.pop().unwrap();
        let mode_str = chunks.pop().unwrap();

        let mode_parser = value_parser!(PlacementSortingModeArg);
        let mode_os_str = OsString::from(mode_str);
        let mode_arg = mode_parser.parse_ref(cmd, None, &mode_os_str)?;

        let sort_order_parser = value_parser!(SortOrderArg);
        let sort_order_os_str = OsString::from(sort_order_str);
        let sort_order_arg = sort_order_parser.parse_ref(cmd, None, &sort_order_os_str)?;

        Ok(PlacementSortingItem {
            mode: mode_arg.to_placement_sorting_mode(),
            sort_order: sort_order_arg.to_sort_order(),
        })
    }
}
