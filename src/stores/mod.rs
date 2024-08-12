/// Stores are for loading/storing different kinds of data.
///
/// Currently, all stores are just simple files, mostly CSV.
/// 
/// Example store backends:
/// * Files (e.g. CSV).
/// * Remote (e.g. REST).
/// * Databases.
/// * Etc.
pub mod parts;
pub mod eda_placements;
pub mod placements;
pub mod part_mappings;

pub mod substitutions;
pub mod load_out;
pub mod assembly_rules;
pub mod csv;