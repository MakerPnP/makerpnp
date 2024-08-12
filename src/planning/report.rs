use serde_with::serde_as;
use serde_with::DisplayFromStr;
use std::path::PathBuf;
use std::collections::BTreeSet;
use tracing::trace;
use std::cmp::Ordering;
use std::fs::File;
use thiserror::Error;
use anyhow::Error;
use std::str::FromStr;
use serde::Serialize;
use std::io::Write;
use crate::planning::design::{DesignName, DesignVariant};
use crate::planning::pcb::PcbKind;
use crate::planning::placement::PlacementStatus;
use crate::planning::project::Project;
use crate::planning::variant::VariantName;
use crate::pnp::object_path::{ObjectPath, UnitPath};
use crate::pnp::part::Part;
use crate::stores::load_out;
use crate::stores::load_out::LoadOutSource;

#[derive(Debug, Error)]
pub enum ReportGenerationError {
    #[error("Unable to save report. cause: {reason:}")]
    UnableToSaveReport { reason: Error },

    #[error("Unable to load items. source: {load_out_source}, error: {reason}")]
    UnableToLoadItems { load_out_source: LoadOutSource, reason: anyhow::Error },
}

pub fn project_generate_report(project: &Project, path: &PathBuf, name: &String, issues: Vec<ProjectReportIssue>) -> Result<(), ReportGenerationError> {

    let mut report = ProjectReport::default();

    report.name.clone_from(&project.name);
    report.issues = issues;
    report.phase_overviews.extend(project.phases.values().map(|phase|{
        PhaseOverview { phase_name: phase.reference.to_string() }
    }));

    let phase_specifications: Vec<PhaseSpecification>  = project.phases.values().try_fold(vec![], |mut results: Vec<PhaseSpecification>, phase | {
        let load_out_items = load_out::load_items(&phase.load_out).map_err(|err|{
            ReportGenerationError::UnableToLoadItems { load_out_source: phase.load_out.clone(), reason: err }
        })?;

        let load_out_assignments = load_out_items.iter().map(|load_out_item|{

            let quantity = project.placements.iter()
                .filter(|(_object_path, placement_state)| {
                    matches!(&placement_state.phase, Some(other_phase_reference) if phase.reference.eq(other_phase_reference))
                        && placement_state.placement.place
                        && load_out_item.manufacturer.eq(&placement_state.placement.part.manufacturer)
                        && load_out_item.mpn.eq(&placement_state.placement.part.mpn)
                })
                .fold(0_u32, | quantity, _placement_state | {
                    quantity + 1
                });

            PhaseLoadOutAssignmentItem {
                feeder_reference: load_out_item.reference.clone(),
                manufacturer: load_out_item.manufacturer.clone(),
                mpn: load_out_item.mpn.clone(),
                quantity,
            }
        }).collect();

        let unit_paths_with_placements = project.placements.iter().fold(BTreeSet::<ObjectPath>::new(), |mut acc, (object_path, placement_state)|{
            if placement_state.placement.place {
                let pcb_unit = object_path.pcb_unit();
                if acc.insert(pcb_unit) {
                    trace!("Phase pcb unit found.  object_path: {}", object_path);
                }
            }
            acc
        });

        let mut operations = vec![];
        if !unit_paths_with_placements.is_empty() {
            let pcbs: Vec<PcbReportItem> = unit_paths_with_placements.iter().find_map(|unit_path|{
                if let Some((key, index)) = unit_path.get(0) {

                    let mut index: usize = index.parse().expect("valid index");
                    // TODO consider if unit paths should use zero-based index (probably!)
                    index -= 1;

                    // Note: the user may not have made any unit assignments yet.
                    let mut unit_assignments = find_unit_assignments(project, unit_path);

                    match PcbKind::try_from(key) {
                        Ok(PcbKind::Panel) => {
                            let pcb = project.pcbs.get(index).unwrap();


                            Some(PcbReportItem::Panel {
                                name: pcb.name.clone(),
                                unit_assignments,
                            })
                        },
                        Ok(PcbKind::Single) => {
                            let pcb = project.pcbs.get(index).unwrap();

                            assert!(unit_assignments.len() <= 1);

                            Some(PcbReportItem::Single {
                                name: pcb.name.clone(),
                                unit_assignment: unit_assignments.pop()
                            })
                        },
                        _ => None,
                    }
                } else {
                    None
                }
            }).into_iter().collect();

            let operation = PhaseOperation::PreparePcbs { pcbs };
            operations.push(operation);
        }

        results.push(PhaseSpecification {
            phase_name: phase.reference.to_string(),
            operations,
            load_out_assignments,
        });

        Ok(results)

    })?;

    report.phase_specifications.extend(phase_specifications);

    let placement_issues = project_report_find_placement_issues(project);
    report.issues.extend(placement_issues);

    project_report_sort_issues(&mut report);

    let report_file_path = build_report_file_path(name, path);

    project_report_save(&report, &report_file_path).map_err(|err|{
        ReportGenerationError::UnableToSaveReport { reason: err }
    })?;

    Ok(())
}

fn project_report_find_placement_issues(project: &Project) -> Vec<ProjectReportIssue> {
    let placement_issues: Vec<ProjectReportIssue> = project.placements.iter().filter_map(|(object_path, placement_state)| {
        match placement_state.phase {
            None if placement_state.status == PlacementStatus::Known => Some(ProjectReportIssue {
                message: "A placement has not been assigned to a phase".to_string(),
                severity: IssueSeverity::Warning,
                kind: IssueKind::UnassignedPlacement { object_path: object_path.clone() },
            }),
            _ => None,
        }
    }).collect();
    placement_issues
}

fn project_report_sort_issues(report: &mut ProjectReport) {
    report.issues.sort_by(|a, b| {
        match (&a.kind, &b.kind) {
            (IssueKind::UnassignedPlacement { .. }, IssueKind::UnassignedPlacement { .. }) => Ordering::Equal,
            (IssueKind::UnassignedPlacement { .. }, _) => Ordering::Less,
            (IssueKind::UnassignedPartFeeder { .. }, _) => Ordering::Greater,
        }
    });
}

fn find_unit_assignments(project: &Project, unit_path: &ObjectPath) -> Vec<PcbUnitAssignmentItem> {
    let unit_assignments = project.unit_assignments.iter().filter_map(|(assignment_unit_path, DesignVariant { design_name, variant_name })| {
        let mut result = None;

        if let Ok(this_unit_path) = &UnitPath::from_str(&unit_path.to_string()) {
            if assignment_unit_path.eq(this_unit_path) {
                result = Some(PcbUnitAssignmentItem {
                    unit_path: unit_path.clone(),
                    design_name: design_name.clone(),
                    variant_name: variant_name.clone(),
                })
            }
        }
        result
    }).collect();

    unit_assignments
}

#[derive(serde::Serialize, Default)]
pub struct ProjectReport {
    pub name: String,
    pub phase_overviews: Vec<PhaseOverview>,
    pub phase_specifications: Vec<PhaseSpecification>,
    pub issues: Vec<ProjectReportIssue>,
}

#[derive(serde::Serialize)]
pub struct PhaseOverview {
    pub phase_name: String,
}

#[derive(Clone, serde::Serialize)]
pub struct PhaseSpecification {
    pub phase_name: String,
    pub operations: Vec<PhaseOperation>,
    pub load_out_assignments: Vec<PhaseLoadOutAssignmentItem>
}

#[serde_as]
#[derive(Clone, serde::Serialize)]
pub struct PcbUnitAssignmentItem {
    #[serde_as(as = "DisplayFromStr")]
    unit_path: ObjectPath,
    design_name: DesignName,
    variant_name: VariantName,
}

#[derive(Clone, serde::Serialize)]
pub enum PcbReportItem {
    // there should be one or more assignments, but the assignment might not have been made yet.
    Panel { name: String, unit_assignments: Vec<PcbUnitAssignmentItem> },
    // there should be exactly one assignment, but the assignment might not have been made yet.
    Single { name: String, unit_assignment: Option<PcbUnitAssignmentItem> },
}

#[derive(Clone, serde::Serialize)]
pub enum PhaseOperation {
    PreparePcbs{ pcbs: Vec<PcbReportItem>}
}

#[derive(Clone, serde::Serialize)]
pub struct PhaseLoadOutAssignmentItem {
    pub feeder_reference: String,
    pub manufacturer: String,
    pub mpn: String,
    pub quantity: u32,
}

#[derive(Clone, serde::Serialize, Debug)]
pub struct ProjectReportIssue {
    pub message: String,
    pub severity: IssueSeverity,
    pub kind: IssueKind,
}

#[derive(Clone, serde::Serialize, Debug)]
pub enum IssueSeverity {
    Severe,
    Warning,
}

#[serde_as]
#[derive(Clone, serde::Serialize, Debug)]
pub enum IssueKind {
    UnassignedPlacement {
        #[serde_as(as = "DisplayFromStr")]
        object_path: ObjectPath
    },
    UnassignedPartFeeder { part: Part },
}

fn build_report_file_path(name: &str, path: &PathBuf) -> PathBuf {
    let mut report_file_path: PathBuf = path.clone();
    report_file_path.push(format!("{}_report.json", name));
    report_file_path
}

fn project_report_save(report: &ProjectReport, report_file_path: &PathBuf) -> anyhow::Result<()> {
    let report_file = File::create(report_file_path)?;
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(report_file, formatter);
    report.serialize(&mut ser)?;

    let mut report_file = ser.into_inner();
    report_file.write(b"\n")?;

    Ok(())
}