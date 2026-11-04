use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::PathBuf;

use gerber::{detect_purpose, GerberFile, GerberFileFunction};
use indexmap::IndexSet;
use itertools::Itertools;
use math::angle::normalize_angle_deg_signed_decimal;
use nalgebra::{Matrix3, Vector2, Vector3};
use pnp::panel::{DesignSizing, PanelSizing};
use pnp::pcb::{PcbUnitIndex, PcbUnitNumber};
use pnp::placement::Placement;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_with::serde_as;
use thiserror::Error;
use tracing::{info, trace};

use crate::design::{DesignIndex, DesignName};
use crate::file;
use crate::project::PcbOperationError;

/// Defines a PCB
///
/// A PCB can have its own gerber files and gerber files for each design, or not at all.
#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Pcb {
    /// A name for this PCB.  e.g. the reference number provided by the PCB fabricator which is often found on the
    /// PCB silk-screen.
    pub name: String,

    /// The count of individual units in the pcb (regardless of the number of designs or design variants)
    ///
    /// This is used to populate the unit_assignments and to define the range used for 'skips' during assembly.
    ///
    /// A value of 0 is invalid
    // TODO validate this after deserializing
    pub units: u16,

    #[serde(skip_serializing_if = "IndexSet::is_empty")]
    #[serde(default)]
    pub design_names: IndexSet<DesignName>,

    /// A hash map of pcb unit number to design index
    /// It's possible that units are not assigned to designs
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub unit_map: BTreeMap<PcbUnitIndex, DesignIndex>,

    /// A set of gerbers that define the panel boundary, usually rectangular, and all the designs within.
    ///
    /// This occurs when you take multiple designs and place them on a single panel or when you design a panel from
    /// scratch.
    ///
    /// This also frequently occurs when you place a single design in the center of a rectangular panel, especially
    /// when the design is not rectangular and/or will not fit in the machines used for assembly.
    ///
    /// panel gerbers are often provided by a 3rd party when you have a 3rd party do the panelization; You give them
    /// the design gerbers, and they give you the panel gerbers.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub pcb_gerbers: Vec<GerberFile>,

    /// In EDA tools like DipTrace, an offset can be specified when exporting gerbers, e.g. (10,5).
    /// Use negative offsets here to relocate the gerber back to (0,0), e.g. (-10, -5)
    #[serde(default)]
    pub gerber_offset: Vector2<f64>,

    /// A set of gerbers for each design used on this PCB
    ///
    /// If the PCB only has one design, with no fiducials, then [`pcb_gerbers`] could be used.
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub design_gerbers: BTreeMap<DesignIndex, Vec<GerberFile>>,

    #[serde(default)]
    pub panel_sizing: PanelSizing,

    /// The orientation of the PCB used for assembly.
    ///
    /// Used in the calculation of placement positions on individual units, and the presentation of gerbers.
    #[serde(default)]
    pub orientation: PcbAssemblyOrientation,
}

#[derive(Error, Debug)]
pub enum PcbError {
    #[error("Unknown design. name: {0:?}")]
    UnknownDesign(DesignName),

    #[error("Unit index {index} is out of range [{min}..{max}] (inclusive)")]
    UnitIndexOutOfRange {
        index: PcbUnitIndex,
        min: PcbUnitIndex,
        max: PcbUnitIndex,
    },
    #[error("Design index {index} is out of range [{min}..{max}] (inclusive)")]
    DesignIndexOutOfRange {
        index: DesignIndex,
        min: DesignIndex,
        max: DesignIndex,
    },

    #[error("Missing unit positioning information for unit {unit}")]
    MissingUnitPositioning { unit: PcbUnitIndex },
}

impl PartialEq for Pcb {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.units == other.units
            && self.design_names == other.design_names
            && self.unit_map == other.unit_map
    }
}

impl Pcb {
    pub fn new(
        name: String,
        units: u16,
        design_names: IndexSet<DesignName>,
        unit_map: BTreeMap<PcbUnitIndex, DesignIndex>,
    ) -> Self {
        Self {
            name,
            units,
            design_names,
            unit_map,
            pcb_gerbers: vec![],
            gerber_offset: Default::default(),
            design_gerbers: Default::default(),
            panel_sizing: Default::default(),
            orientation: PcbAssemblyOrientation::default(),
        }
    }

    /// returns a transform matrix that can be applied to a placement to position it on the unit.
    /// See [`PcbUnitTransform`]
    pub fn build_unit_transform(
        &self,
        pcb_unit_index: PcbUnitIndex,
        orientation: &PcbSideAssemblyOrientation,
    ) -> Result<PcbUnitTransform, PcbError> {
        let design_index = *self
            .unit_map
            .get(&pcb_unit_index)
            .ok_or(PcbError::UnitIndexOutOfRange {
                index: pcb_unit_index,
                min: 0,
                max: self.units,
            })?;
        let design = self
            .panel_sizing
            .design_sizings
            .get(design_index as usize)
            .ok_or(PcbError::DesignIndexOutOfRange {
                index: design_index,
                min: 0,
                max: self.design_names.len(),
            })?;

        let pcb_unit_positioning = self
            .panel_sizing
            .pcb_unit_positionings
            .get(pcb_unit_index as usize)
            .ok_or(PcbError::MissingUnitPositioning {
                unit: pcb_unit_index,
            })?;

        Ok(PcbUnitTransform {
            unit_offset: pcb_unit_positioning.offset,
            unit_rotation: pcb_unit_positioning.rotation,

            design_sizing: design.clone(),

            orientation: orientation.clone(),

            panel_size: self.panel_sizing.size,
        })
    }

    /// returns true if the design exists
    pub fn has_design(&mut self, design_name: &DesignName) -> bool {
        self.design_names
            .iter()
            .any(|candidate| candidate.eq(design_name))
    }

    /// returns the design index if the design exists, otherwise None
    pub fn design_index(&mut self, design_name: &DesignName) -> Option<DesignIndex> {
        self.design_names
            .iter()
            .position(|candidate| candidate.eq(design_name))
    }

    pub fn unique_designs_iter(&self) -> impl Iterator<Item = &DesignName> {
        self.design_names.iter()
    }

    /// If `design` is None, then the changes are applied to the PCB, otherwise they are applied to the design.
    /// returns a [`Result`] containing the modified state of the PCB, or an error.
    pub fn update_gerbers(
        &mut self,
        design: Option<DesignName>,
        files: Vec<(PathBuf, Option<GerberFileFunction>)>,
    ) -> Result<bool, PcbError> {
        let gerbers = self.gerbers_for_pcb_or_design(design)?;
        let mut modified = false;

        let mut new_gerbers = gerbers.clone();

        for (file, optional_function) in files {
            new_gerbers.push(GerberFile {
                file,
                function: optional_function,
            });
        }

        for gerber in new_gerbers.iter_mut() {
            let new_purpose = detect_purpose(&gerber.file).ok();
            gerber.function = new_purpose;
        }

        if gerbers.iter().ne(new_gerbers.iter()) {
            info!("Updated gerbers. old:\n{:?}, new:\n{:?}", gerbers, new_gerbers);
            *gerbers = new_gerbers;
            modified |= true;
        }

        Ok(modified)
    }

    /// If `design` is None, then the changers are applied to the PCB, otherwise they are applied to the design.
    /// returns a [`Result`] containing the modified state of the PCB, or an error.
    // FUTURE currently this silently ignore paths that were not in the list, but perhaps we should return a result to
    //        allow the user to be informed which files could not be removed.
    pub fn remove_gerbers(
        &mut self,
        design: Option<DesignName>,
        files: Vec<PathBuf>,
    ) -> Result<(bool, Vec<PathBuf>), PcbError> {
        let gerbers = self.gerbers_for_pcb_or_design(design)?;
        let mut modified = false;
        let mut unremoved_files = files;

        unremoved_files.retain(|file| {
            let mut should_remove = false;

            gerbers.retain(|candidate| {
                should_remove = candidate.file.eq(file);
                if should_remove {
                    trace!("Removing gerber file. file: {:?}", file);
                }
                modified |= should_remove;

                !should_remove
            });

            !should_remove
        });

        Ok((modified, unremoved_files))
    }

    fn gerbers_for_pcb_or_design(&mut self, design: Option<DesignName>) -> Result<&mut Vec<GerberFile>, PcbError> {
        let gerbers = match design {
            Some(design_name) => {
                let design_index = self
                    .design_index(&design_name)
                    .ok_or(PcbError::UnknownDesign(design_name))?;

                self.design_gerbers
                    .entry(design_index)
                    .or_default()
            }
            None => &mut self.pcb_gerbers,
        };
        Ok(gerbers)
    }

    pub fn apply_file_functions(&mut self, file_functions: Vec<(PathBuf, Option<GerberFileFunction>)>) -> bool {
        fn apply_file_function(function: Option<GerberFileFunction>, gerber: &mut GerberFile) -> bool {
            let old_function = gerber.function;
            gerber.function = function;

            old_function != gerber.function
        }

        let mut modified = false;
        for (path, function) in file_functions {
            if let Some(gerber) = self
                .pcb_gerbers
                .iter_mut()
                .find(|candidate| candidate.file.eq(&path))
            {
                modified |= apply_file_function(function, gerber);
            }

            for design_gerbers in self.design_gerbers.values_mut() {
                if let Some(gerber) = design_gerbers
                    .iter_mut()
                    .find(|candidate| candidate.file.eq(&path))
                {
                    modified |= apply_file_function(function, gerber);
                }
            }
        }
        modified
    }
}

pub fn create_pcb(
    name: String,
    units: u16,
    unit_to_design_name_map: BTreeMap<PcbUnitNumber, DesignName>,
) -> Result<Pcb, PcbOperationError> {
    info!("Creating PCB. name: '{}'", name);

    let (design_names, unit_to_design_index_mapping) = build_unit_to_design_index_mappping(unit_to_design_name_map);

    let design_count = design_names.len();
    let mut pcb = Pcb::new(name, units, design_names, unit_to_design_index_mapping);
    pcb.panel_sizing
        .ensure_unit_positionings(units);
    pcb.panel_sizing
        .ensure_design_sizings(design_count);

    Ok(pcb)
}

/// A PCB, from its document's contents, as read for a core.
pub fn from_contents(contents: &str) -> Result<Pcb, serde_json::Error> {
    file::from_str::<Pcb>(contents).map(ensure_panel_sizing)
}

fn ensure_panel_sizing(mut pcb: Pcb) -> Pcb {
    // TODO can we somehow integrate this block into the deserialization so we don't have to do it explicitly?
    pcb.panel_sizing
        .ensure_design_sizings(pcb.design_names.len());
    pcb.panel_sizing
        .ensure_unit_positionings(pcb.units);

    pcb
}

pub fn build_unit_to_design_index_mappping(
    unit_to_design_name_map: BTreeMap<PcbUnitNumber, DesignName>,
) -> (IndexSet<DesignName>, BTreeMap<PcbUnitIndex, DesignIndex>) {
    trace!("unit_to_design_name_map: {:?}", unit_to_design_name_map);

    // 'Intern' the DesignNames
    let mut unit_to_design_index_mapping: BTreeMap<PcbUnitIndex, DesignIndex> = BTreeMap::new();
    let mut unique_strings: Vec<DesignName> = Vec::new();
    let mut design_names: IndexSet<DesignName> = IndexSet::new();

    for (pcb_unit_number, design) in unit_to_design_name_map {
        // Insert into unique list if not seen
        let design_index = if let Some(position) = unique_strings
            .iter()
            .position(|s| s == &design)
        {
            position
        } else {
            unique_strings.push(design.clone());
            unique_strings.len() - 1
        };

        design_names.insert(design.clone());
        let pcb_unit_index = pcb_unit_number - 1;
        unit_to_design_index_mapping.insert(pcb_unit_index, design_index);
    }

    info!("Added designs to PCB. design: [{}]", unique_strings.iter().join(", "));
    trace!("unit_to_design_index_mapping: {:?}", unit_to_design_index_mapping);

    (design_names, unit_to_design_index_mapping)
}

/// A transform matrix that can be applied to a placement to position it on the unit.
/// transform order:
/// DesignSizing::placement_offset,
/// -DesignSizing::origin,
/// unit_rotation,
/// unit_offset,
/// +DesignSizing::origin,
/// panel centering,
/// orientation rotation,
/// orientation flip,
/// panel un-centering,
/// shift to positive coordinates
#[derive(Debug)]
pub struct PcbUnitTransform {
    /// (x,y)
    pub unit_offset: Vector2<f64>,
    /// rotation in degrees, positive is anti-clockwise
    pub unit_rotation: Decimal,

    pub design_sizing: DesignSizing,

    pub orientation: PcbSideAssemblyOrientation,

    pub panel_size: Vector2<f64>,
}

impl PcbUnitTransform {
    #[rustfmt::skip]
    pub fn to_matrix(&self) -> Matrix3<f64> {
        // Start with identity matrix
        let mut matrix = Matrix3::identity();

        // Translate placement offset (which for an EDA offset of 10,10 would be specifed at -10,-10, no need to invert the sign.
        // Note: /PLACEMENT/ offset *not* /GERBER/ offset here.
        let translate_offset = Matrix3::new(
            1.0, 0.0, self.design_sizing.placement_offset.x,
            0.0, 1.0, self.design_sizing.placement_offset.y,
            0.0, 0.0, 1.0,
        );
        matrix = translate_offset * matrix;

        // Translate to design origin (negative of design_sizing.origin)
        let translation_to_origin = Matrix3::new(
            1.0, 0.0, -self.design_sizing.origin.x,
            0.0, 1.0, -self.design_sizing.origin.y,
            0.0, 0.0, 1.0,
        );
        matrix = translation_to_origin * matrix;

        // Apply unit rotation (anti-clockwise positive degrees)
        let unit_rotation_radians = self.unit_rotation.to_f64().unwrap().to_radians();

        let cos_theta = unit_rotation_radians.cos();
        let sin_theta = unit_rotation_radians.sin();
        let rotation = Matrix3::new(
            cos_theta, -sin_theta, 0.0,
            sin_theta, cos_theta, 0.0,
            0.0, 0.0, 1.0
        );
        matrix = rotation * matrix;

        // Apply unit offset
        let unit_offset = Matrix3::new(
            1.0,0.0,self.unit_offset.x,
            0.0,1.0,self.unit_offset.y,
            0.0,0.0,1.0,
        );
        matrix = unit_offset * matrix;

        // Translate back from origin (positive of design_sizing.origin)
        let translation_from_origin = Matrix3::new(
            1.0, 0.0, self.design_sizing.origin.x,
            0.0, 1.0, self.design_sizing.origin.y,
            0.0, 0.0, 1.0,
        );
        matrix = translation_from_origin * matrix;

        // Translate to panel center
        let panel_center: Vector2<f64> = self.panel_size / 2.0;
        let translation_to_panel_origin = Matrix3::new_translation(&-panel_center);
        let translation_from_panel_origin = Matrix3::new_translation(&panel_center);
        matrix = translation_to_panel_origin * matrix;

        // Apply orientation flipping
        if !matches!(self.orientation.flip, PcbAssemblyFlip::None) {
            let flip_matrix: Matrix3<f64> = self.orientation.flip.into();
            trace!("flip_matrix: {:?}", flip_matrix);
            matrix = flip_matrix * matrix;
        }

        // Apply orientation rotation (anti-clockwise positive degrees)
        let orientation_radians = self.orientation.rotation.to_f64().unwrap().to_radians();
        let cos_theta = orientation_radians.cos();
        let sin_theta = orientation_radians.sin();
        let orientation_rotation = Matrix3::new(
            cos_theta, -sin_theta, 0.0,
            sin_theta, cos_theta, 0.0,
            0.0, 0.0, 1.0
        );
        matrix = orientation_rotation * matrix;

        // Translate from panel center
        matrix = translation_from_panel_origin * matrix;

        // Calculate bounding box for the rotated panel to ensure positive coordinates
        let panel_corners = [
            Vector2::new(0.0, 0.0),
            Vector2::new(self.panel_size.x, 0.0),
            Vector2::new(self.panel_size.x, self.panel_size.y),
            Vector2::new(0.0, self.panel_size.y),
        ];
        trace!("panel_corners: {:?}", panel_corners);

        // Rotate the corners using our current transformation matrix 
        // (limiting to just the rotation part to avoid double-translation)
        let rotated_corners: Vec<Vector3<f64>> = panel_corners
            .iter()
            .map(|&corner| {
                // Apply only the rotation part of the matrix
                let rotation_only = orientation_rotation *
                    translation_to_panel_origin *
                    Vector3::new(corner.x, corner.y, 1.0);

                // Apply translation back from panel center
                translation_from_panel_origin * rotation_only
            })
            .collect();

        trace!("rotated_corners: {:?}", rotated_corners);

        // Find minimum x and y to shift by

        let shift = Vector2::new(
            rotated_corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
            rotated_corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min)
        );
        trace!("shift: {:?}", shift);

        // Add translation to shift all points to positive quadrant
        let shift_to_positive = Matrix3::new(
            1.0, 0.0, -shift.x,
            0.0, 1.0, -shift.y,
            0.0, 0.0, 1.0
        );
        matrix = shift_to_positive * matrix;

        trace!("PcbUnitTransform {:?}, matrix: {:?}", self, matrix);

        matrix
    }

    pub fn apply_to_placement_matrix(&self, placement: &Placement) -> UnitPlacementPosition {
        // Get the transformation matrix
        let transform_matrix = self.to_matrix();

        // Convert placement position to homogeneous coordinates (x, y, 1)
        let position = nalgebra::Vector3::new(
            placement.x.to_f64().unwrap_or(0.0),
            placement.y.to_f64().unwrap_or(0.0),
            1.0,
        );

        // Apply the transformation matrix to the position
        let transformed_position = transform_matrix * position;

        //
        // Handle rotation
        //

        let flipped_rotation = match &self.orientation.flip {
            PcbAssemblyFlip::None => placement.rotation,
            PcbAssemblyFlip::Pitch => dec!(180.0) - placement.rotation,
            PcbAssemblyFlip::Roll => dec!(360.0) - placement.rotation,
        };

        let new_rotation = flipped_rotation + self.orientation.rotation + self.unit_rotation;

        // Normalize rotation to be within -180 to 180 degrees
        let normalized_rotation = normalize_angle_deg_signed_decimal(new_rotation).normalize();

        trace!(
            "placement_rotation: {}, self.orientation.rotation: {}, unit_rotation: {}",
            placement.rotation,
            self.orientation.rotation,
            self.unit_rotation
        );
        trace!(
            "new_rotation: {}, normalized rotation: {}",
            new_rotation,
            normalized_rotation
        );

        let x = Decimal::try_from(transformed_position.x).unwrap_or_default();
        let y = Decimal::try_from(transformed_position.y).unwrap_or_default();
        let rotation = Decimal::try_from(normalized_rotation).unwrap_or_default();

        UnitPlacementPosition {
            x,
            y,
            rotation,
        }
    }
}

/// This describes the position of a placement on a panel unit, after the placement coordinates and rotation have been
/// transformed by a PcbUnitTransform.
///
/// Uses the same coordinate scheme as Placement.
#[derive(
    Debug,
    Default,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    PartialEq,
    Eq,
    PartialOrd
)]
pub struct UnitPlacementPosition {
    /// Positive = Right
    pub x: Decimal,
    /// Positive = Up
    pub y: Decimal,

    /// Positive values indicate anti-clockwise rotation
    /// Range is >-180 to +180 degrees
    pub rotation: Decimal,
}

/// Defines the orientation for a PCB as positioned in the machine, for each side.
///
/// Used when transforming placement coordinates and component rotations.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct PcbAssemblyOrientation {
    pub top: PcbSideAssemblyOrientation,
    pub bottom: PcbSideAssemblyOrientation,
}

/// The default orientation for the bottom is to hold the PCB by the left and right sides, then flip it over top-to-bottom
/// i.e. pitch 180 degrees, without rotating the PCB on either roll or yaw axis.
///
/// In the physical world this pitch-flipping happens in 3D space, but the coordinates are in 2D space.
/// and is referred to as y-mirroring.  similarly, a left-to-right roll flip would be x-mirroring in 2D space.
impl Default for PcbAssemblyOrientation {
    fn default() -> Self {
        Self {
            top: PcbSideAssemblyOrientation {
                flip: PcbAssemblyFlip::None,
                rotation: Decimal::from(0),
            },
            bottom: PcbSideAssemblyOrientation {
                flip: PcbAssemblyFlip::Pitch,
                rotation: Decimal::from(0),
            },
        }
    }
}

/// Specifies how the PCB should be positioned in the machine.
///
/// Transform order: flip (mirroring), rotation
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct PcbSideAssemblyOrientation {
    pub flip: PcbAssemblyFlip,
    /// In degrees, counter-clockwise positive
    pub rotation: Decimal,
}

/// How to 'flip' a physical PCB.
///
/// We specifically do NOT use terms like 'mirror' or 'reflect' here because that is not what happens to the physical
/// PCB in the real world.
///
/// Additionally, since there are inconsistencies between 'flip-along', 'flip-over', 'flip-about' and 'mirroring' and
/// 'reflection' we use 'pitch' and 'roll' since they are unambiguous.
///
/// For clarity, this table describes the various different terminology used to describe flipping operations and how they relate to each other.
///
/// | Term           | Flip-over/about | Flip-along | Mirrored/Reflected axis | Matrix                             | Hold           | Result                       | Coordinate in | Coordinate out |
/// | -------------- | --------------- | ---------- | ----------------------- | ---------------------------------- |--------------- | ---------------------------- | ------------- | -------------- |
/// | Pitch flip     | x               | y          | y                       | [( 1, 0, 0), (0,-1, 0), (0, 0, 1)] | Left and right | Top edge becomes bottom edge | (1,1)         | ( 1,-1)        |
/// | Roll flip      | y               | x          | x                       | [(-1, 0, 0), (0, 1, 0), (0, 0, 1)] | Top and bottom | Left edge becomes right edge | (1,1)         | (-1, 1)        |
#[derive(Debug, serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq)]
pub enum PcbAssemblyFlip {
    None,
    Pitch, // Flip about the X axis (negates Y)
    Roll,  // Flip about the Y axis (negates X)
}

impl From<PcbAssemblyFlip> for Matrix3<f64> {
    fn from(flip: PcbAssemblyFlip) -> Self {
        match flip {
            PcbAssemblyFlip::None => Matrix3::identity(),
            PcbAssemblyFlip::Pitch => Matrix3::new(1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0),
            PcbAssemblyFlip::Roll => Matrix3::new(-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
        }
    }
}

#[cfg(test)]
mod pcb_assembly_flip_tests {
    use nalgebra::{Matrix3, Point2, Vector3};
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(
        PcbAssemblyFlip::None,
        Matrix3::identity(),
        Point2::new(1.0, 2.0),
        Point2::new(1.0, 2.0)
    )]
    #[case(
        PcbAssemblyFlip::Roll,
        Matrix3::new(-1.0, 0.0, 0.0,
                      0.0, 1.0, 0.0,
                      0.0, 0.0, 1.0),
        Point2::new(1.0, 2.0),
        Point2::new(-1.0, 2.0)
    )]
    #[case(
        PcbAssemblyFlip::Pitch,
        Matrix3::new(1.0, 0.0, 0.0,
                     0.0, -1.0, 0.0,
                     0.0, 0.0, 1.0),
        Point2::new(1.0, 2.0),
        Point2::new(1.0, -2.0)
    )]
    fn test_pcb_assembly_flip_matrix(
        #[case] flip: PcbAssemblyFlip,
        #[case] expected_matrix: Matrix3<f64>,
        #[case] input: Point2<f64>,
        #[case] expected_output: Point2<f64>,
    ) {
        let flip_matrix: Matrix3<f64> = flip.into();
        assert_eq!(flip_matrix, expected_matrix);

        let input_vec = Vector3::new(input.x, input.y, 1.0);
        let result = flip_matrix * input_vec;
        let result_point = Point2::new(result.x, result.y);

        assert!(
            (result_point.coords - expected_output.coords)
                .abs()
                .max()
                < 1e-9,
            "expected {:?}, got {:?}",
            expected_output,
            result_point
        );
    }
}
