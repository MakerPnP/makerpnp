use regex::Regex;
use tracing::{info, trace};
use thiserror::Error;
use std::collections::BTreeSet;
use crate::planning::reference::Reference;
use crate::pnp;
use crate::pnp::load_out::LoadOutItem;
use crate::pnp::part::Part;
use crate::stores::load_out;
use crate::stores::load_out::{LoadOutOperationError, LoadOutSource};

pub fn add_parts_to_load_out(load_out_source: &LoadOutSource, parts: BTreeSet<Part>) -> Result<(), LoadOutOperationError<anyhow::Error>> {
    
    load_out::perform_load_out_operation(load_out_source, | load_out_items| {
        for part in parts.iter() {
            trace!("Checking for part in load_out. part: {:?}", part);
    
            let matched = pnp::load_out::find_load_out_item_by_part(load_out_items, part);
            
            if matched.is_some() {
                continue
            }
            
            let load_out_item = LoadOutItem {
                reference: "".to_string(),
                manufacturer: part.manufacturer.clone(),
                mpn: part.mpn.clone(),
            };

            info!("Adding part to load_out. part: {:?}", part);
            load_out_items.push(load_out_item)
        }
        
        Ok(())
    })
}

#[derive(Error, Debug)]
pub enum FeederAssignmentError {
    #[error("No matching part; patterns must match exactly one part. manufacturer: {manufacturer}, mpn: {mpn}")]
    NoMatchingPart { manufacturer: Regex, mpn: Regex },

    #[error("Multiple matching parts; patterns must match exactly one part. manufacturer: {manufacturer}, mpn: {mpn}")]
    MultipleMatchingParts { manufacturer: Regex, mpn: Regex },
}

pub fn assign_feeder_to_load_out_item(load_out_source: LoadOutSource, feeder_reference: Reference, manufacturer: Regex, mpn: Regex) -> Result<(), LoadOutOperationError<FeederAssignmentError>> {

    let part = load_out::perform_load_out_operation(&load_out_source, | load_out_items| {
        let mut items: Vec<_> = load_out_items.iter_mut().filter(|item| {
            manufacturer.is_match(&item.manufacturer)
                && mpn.is_match(&item.mpn)
        }).collect();

        match items.len() {
            0 => Err(FeederAssignmentError::NoMatchingPart { manufacturer: manufacturer.clone(), mpn: mpn.clone() }),
            1 => Ok(()),
            _ => Err(FeederAssignmentError::MultipleMatchingParts { manufacturer: manufacturer.clone(), mpn: mpn.clone() }),
        }?;

        let item = items.pop().unwrap();
        item.reference = feeder_reference.to_string();

        let part = Part { manufacturer: item.manufacturer.to_string(), mpn: item.mpn.to_string() };
        Ok(part)
        
    })?;
    
    info!("Assigned feeder to load-out item. feeder: {}, part: {:?}", feeder_reference, part);

    Ok(())
}