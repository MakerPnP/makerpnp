pub mod diptrace;
pub mod kicad;

pub mod assembly_variant;
pub mod placement;
pub mod substitution;
pub mod criteria;

#[derive(Debug)]
pub enum EdaTool {
    DipTrace,
    KiCad,
}
