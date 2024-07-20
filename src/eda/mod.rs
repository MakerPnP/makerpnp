pub mod diptrace;
pub mod kicad;

pub mod assembly_variant;
pub mod eda_placement;
pub mod eda_substitution;

// TODO consider removing `eda_` prefix from `eda_placement` and `eda_substitution` modules

#[derive(Debug)]
pub enum EdaTool {
    DipTrace,
    KiCad,
}
