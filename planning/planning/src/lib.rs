// Used by the planner's core, so, as the core, it does no IO, see `clippy.toml`.
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

pub mod actions;
pub mod design;

pub mod library;
pub mod operation;
pub mod operation_history;
pub mod part;
pub mod pcb;
pub mod phase;
pub mod placement;
pub mod process;
pub mod project;

pub mod report;
pub mod variant;

pub mod file;

#[cfg(test)]
pub mod tests;
