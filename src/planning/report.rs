use serde_with::serde_as;
use serde_with::DisplayFromStr;
use std::path::PathBuf;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tracing::{info, trace};
use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::fs::File;
use thiserror::Error;
use anyhow::Error;
use serde::Serialize;
use std::io::Write;
use crate::planning::design::{DesignName, DesignVariant};
use crate::planning::pcb::PcbKind;
use crate::planning::placement::PlacementStatus;
use crate::planning::project::Project;
use crate::planning::reference::Reference;
use crate::planning::variant::VariantName;
use crate::pnp::load_out::LoadOutItem;
use crate::pnp::object_path::ObjectPath;
use crate::pnp::part::Part;
use crate::util::sorting::SortOrder;

#[derive(Debug, Error)]
pub enum ReportGenerationError {
    #[error("Unable to save report. cause: {reason:}")]
    UnableToSaveReport { reason: Error },
}

// FUTURE add a test to ensure that duplicate issues are not added to the report.
//        currently a BTreeSet is used to prevent duplicate issues.

pub fn project_generate_report(project: &Project, path: &PathBuf, name: &String, phase_load_out_items_map: &BTreeMap<Reference, Vec<LoadOutItem>>, issues: &mut BTreeSet<ProjectReportIssue>) -> Result<(), ReportGenerationError> {

    let mut report = ProjectReport::default();

    report.name.clone_from(&project.name);
    if project.pcbs.is_empty() {
        issues.insert(ProjectReportIssue {
            message: "No PCBs have been assigned to the project.".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        });
    }

    if !project.phases.is_empty() {
        report.phase_overviews.extend(project.phase_orderings.iter().map(|reference| {
            let phase = project.phases.get(reference).unwrap();

            PhaseOverview { 
                phase_name: phase.reference.to_string(),
                process: phase.process.to_string(),
            }
        }));
    } else {
        issues.insert(ProjectReportIssue {
            message: "No phases have been created.".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPhasesCreated,
        });
    }

    // generate issues for invalid unit assignments
    for (object_path, _design_variant) in project.unit_assignments.iter() {

        let mut pcb_kind_counts: HashMap<PcbKind, usize> = Default::default();
        for pcb in project.pcbs.iter() {
            pcb_kind_counts.entry(pcb.kind.clone())
                .and_modify(|e| { *e += 1 })
                .or_insert(1);       
        }

        if let Some((pcb_kind, index)) = object_path.pcb_kind_and_index() {

            let issue = match pcb_kind_counts.entry(pcb_kind.clone()) {
                Entry::Occupied(entry) => {
                    let count = *entry.get();
                    if index > count {
                        Some(ProjectReportIssue {
                            message: "Invalid unit assignment, index out of range.".to_string(),
                            severity: IssueSeverity::Severe,
                            kind: IssueKind::InvalidUnitAssignment { object_path: object_path.clone() },
                        })           
                    } else {
                        None
                    }
                    
                }
                Entry::Vacant(_) => Some(ProjectReportIssue {
                    message: "Invalid unit assignment, no pcbs match the assignment.".to_string(),
                    severity: IssueSeverity::Severe,
                    kind: IssueKind::InvalidUnitAssignment { object_path: object_path.clone() },
                })
            };
            
            if let Some(issue) = issue {
                issues.insert(issue);
            }
        }
    }

    let phase_specifications: Vec<PhaseSpecification>  = project.phase_orderings.iter().try_fold(vec![], |mut results: Vec<PhaseSpecification>, reference | {
        let phase = project.phases.get(reference).unwrap();
        
        let load_out_items = phase_load_out_items_map.get(reference).unwrap();

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
                if let Some((kind, mut index)) = unit_path.pcb_kind_and_index() {

                    // TODO consider if unit paths should use zero-based index
                    index -= 1;

                    // Note: the user may not have made any unit assignments yet.
                    let mut unit_assignments = find_unit_assignments(project, unit_path);

                    match kind {
                        PcbKind::Panel => {
                            let pcb = project.pcbs.get(index).unwrap();


                            Some(PcbReportItem::Panel {
                                name: pcb.name.clone(),
                                unit_assignments,
                            })
                        },
                        PcbKind::Single => {
                            let pcb = project.pcbs.get(index).unwrap();

                            assert!(unit_assignments.len() <= 1);

                            Some(PcbReportItem::Single {
                                name: pcb.name.clone(),
                                unit_assignment: unit_assignments.pop()
                            })
                        },
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

    project_report_add_placement_issues(project, issues);
    let mut issues: Vec<ProjectReportIssue> = issues.iter().cloned().collect();

    project_report_sort_issues(&mut issues);
    
    for issue in issues.iter() {
        info!("Issue detected. severity: {:?}, message: '{}', kind: {:?}", issue.severity, issue.message, issue.kind );
    }
    
    report.issues = issues;

    let report_file_path = build_report_file_path(name, path);

    project_report_save(&report, &report_file_path).map_err(|err|{
        ReportGenerationError::UnableToSaveReport { reason: err }
    })?;

    Ok(())
}

fn project_report_add_placement_issues(project: &Project, issues: &mut BTreeSet<ProjectReportIssue>) {
    for (object_path, _placement_state) in project.placements.iter().filter(|(_object_path, placement_state)| {
        placement_state.phase.is_none() && placement_state.status == PlacementStatus::Known
    }) {
        issues.insert(ProjectReportIssue {
            message: "A placement has not been assigned to a phase".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::UnassignedPlacement { object_path: object_path.clone() },
        });
    }
}

fn project_report_sort_issues(issues: &mut [ProjectReportIssue]) {
    issues.sort_by(|a, b| {

        let sort_orderings = &[("severity", SortOrder::Desc), ("kind", SortOrder::Asc), ("message", SortOrder::Asc)];
        
        sort_orderings.iter().fold( Ordering::Equal, | mut acc, (&ref mode, sort_order) | {
            if !matches!(acc, Ordering::Equal) {
                return acc
            }

            fn kind_ordinal(kind: &IssueKind) -> usize {
                match kind {
                    IssueKind::NoPcbsAssigned => 0,
                    IssueKind::NoPhasesCreated => 1,
                    IssueKind::InvalidUnitAssignment { .. } => 2,
                    IssueKind::UnassignedPlacement { .. } => 3,
                    IssueKind::UnassignedPartFeeder { .. } => 4,
                }   
            }
            fn severity_ordinal(severity: &IssueSeverity) -> usize {
                match severity {
                    IssueSeverity::Warning => 0,
                    IssueSeverity::Severe => 1,
                }   
            }
            
            acc = match mode {
                "kind" => {
                    let a_ordinal = kind_ordinal(&a.kind); 
                    let b_ordinal = kind_ordinal(&b.kind);
                    let ordinal_ordering = a_ordinal.cmp(&b_ordinal);
                    
                    match ordinal_ordering {
                        Ordering::Less => ordinal_ordering,
                        Ordering::Equal => {
                            match (&a.kind, &b.kind) {
                                (IssueKind::InvalidUnitAssignment { object_path: object_path_a }, IssueKind::InvalidUnitAssignment { object_path: object_path_b }) =>
                                    object_path_a.cmp(object_path_b),
                                (IssueKind::UnassignedPlacement { object_path: object_path_a }, IssueKind::UnassignedPlacement { object_path: object_path_b }) =>
                                    object_path_a.cmp(object_path_b),
                                (IssueKind::UnassignedPartFeeder { part: part_a }, IssueKind::UnassignedPartFeeder { part: part_b}) =>
                                    part_a.cmp(part_b),
                                _ => ordinal_ordering,
                            }
                        }
                        Ordering::Greater => ordinal_ordering,
                    }
                },
                "message" => a.message.cmp(&b.message),
                "severity" => {
                    let a_ordinal = severity_ordinal(&a.severity);
                    let b_ordinal = severity_ordinal(&b.severity);
                    let ordinal_ordering = a_ordinal.cmp(&b_ordinal);
                    ordinal_ordering
                },
                _ => unreachable!()
            };

            match sort_order {
                SortOrder::Asc => acc,
                SortOrder::Desc => {
                    acc.reverse()
                },
            }
        })
    });
}

#[cfg(test)]
mod report_issue_sorting {
    use std::str::FromStr;
    use crate::planning::report::{IssueKind, IssueSeverity, project_report_sort_issues, ProjectReportIssue};
    use crate::pnp::object_path::ObjectPath;
    use crate::pnp::part::Part;

    #[test]
    pub fn sort_by_severity_with_equal_message_and_kind() {
        // given 
        let issue1 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        };
        let issue2 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::NoPcbsAssigned,
        };

        let mut issues: Vec<ProjectReportIssue> = vec![
            issue2.clone(), issue1.clone(),
        ];
        let expected_issues: Vec<ProjectReportIssue> = vec![
            issue1.clone(), issue2.clone(),
        ];

        // when
        project_report_sort_issues(&mut issues);

        // then
        assert_eq!(&issues, &expected_issues);
    }

    #[test]
    pub fn sort_by_message_with_severity_and_kind() {
        // given 
        let issue1 = ProjectReportIssue {
            message: "MESSAGE_1".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        };
        let issue2 = ProjectReportIssue {
            message: "MESSAGE_2".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        };

        let mut issues: Vec<ProjectReportIssue> = vec![
            issue2.clone(), issue1.clone(),
        ];
        let expected_issues: Vec<ProjectReportIssue> = vec![
            issue1.clone(), issue2.clone(),
        ];

        // when
        project_report_sort_issues(&mut issues);

        // then
        assert_eq!(&issues, &expected_issues);
    }

    #[test]
    pub fn sort_by_kind_with_equal_message_and_severity() {
        // given 
        let issue1 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        };
        let issue2 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPhasesCreated,
        };
        let issue3 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::InvalidUnitAssignment { object_path: ObjectPath::from_str("panel=1").expect("always ok") },
        };
        let issue4 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::InvalidUnitAssignment { object_path: ObjectPath::from_str("panel=2").expect("always ok") },
        };
        let issue5 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPlacement { object_path: ObjectPath::from_str("panel=1::unit=1::ref_des=R1").expect("always ok") },
        };
        let issue6 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPlacement { object_path: ObjectPath::from_str("panel=1::unit=1::ref_des=R2").expect("always ok") },
        };
        let issue7 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPartFeeder { part: Part { manufacturer: "MFR1".to_string(), mpn: "MPN1".to_string() } },
        };
        let issue8 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPartFeeder { part: Part { manufacturer: "MFR1".to_string(), mpn: "MPN2".to_string() } },
        };
        let issue9 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPartFeeder { part: Part { manufacturer: "MFR2".to_string(), mpn: "MPN1".to_string() } },
        };
        
        let mut issues: Vec<ProjectReportIssue> = vec![
            issue9.clone(), issue8.clone(), issue7.clone(), 
            issue6.clone(), issue5.clone(), issue4.clone(),
            issue3.clone(), issue2.clone(), issue1.clone(),
        ];
        let expected_issues: Vec<ProjectReportIssue> = vec![
            issue1.clone(), issue2.clone(), issue3.clone(),
            issue4.clone(), issue5.clone(), issue6.clone(),
            issue7.clone(), issue8.clone(), issue9.clone(),
        ];
        
        // when
        project_report_sort_issues(&mut issues);
        
        // then
        assert_eq!(&issues, &expected_issues);
    }

    #[test]
    pub fn sort_by_severity_kind_and_message() {
        // given 
        let issue1 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        };
        let issue2 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::NoPcbsAssigned,
        };
        let issue3 = ProjectReportIssue {
            message: "DIFFERENT".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::NoPcbsAssigned,
        };
        
        let issue4 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPhasesCreated,
        };
        let issue5 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::NoPhasesCreated,
        };
        let issue6 = ProjectReportIssue {
            message: "DIFFERENT".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::NoPhasesCreated,
        };

        let mut issues: Vec<ProjectReportIssue> = vec![
            issue1.clone(), issue2.clone(), issue3.clone(),
            issue4.clone(), issue5.clone(), issue6.clone(),
        ];
        let expected_issues: Vec<ProjectReportIssue> = vec![
            issue1.clone(), issue4.clone(), issue3.clone(),
            issue2.clone(), issue6.clone(), issue5.clone(),
        ];

        // when
        project_report_sort_issues(&mut issues);

        // then
        assert_eq!(&issues, &expected_issues);
    }
}

fn find_unit_assignments(project: &Project, unit_path: &ObjectPath) -> Vec<PcbUnitAssignmentItem> {
    let unit_assignments = project.unit_assignments.iter().filter_map(|(assignment_unit_path, DesignVariant { design_name, variant_name })| {
        let mut result = None;

        if assignment_unit_path.eq(unit_path) {
            result = Some(PcbUnitAssignmentItem {
                unit_path: unit_path.clone(),
                design_name: design_name.clone(),
                variant_name: variant_name.clone(),
            })
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
    /// A list of unique issues.
    /// Note: Using a Vec doesn't prevent duplicates, duplicates must be filtered before adding them.
    pub issues: Vec<ProjectReportIssue>,
}

#[derive(serde::Serialize)]
pub struct PhaseOverview {
    pub phase_name: String,
    pub process: String,
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

// FUTURE implement `Display` and improve info logging
#[derive(Clone, serde::Serialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectReportIssue {
    pub message: String,
    pub severity: IssueSeverity,
    pub kind: IssueKind,
}

#[derive(Clone, serde::Serialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    Severe,
    Warning,
}

#[serde_as]
#[derive(Clone, serde::Serialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueKind {
    NoPcbsAssigned,
    NoPhasesCreated,
    InvalidUnitAssignment {
        #[serde_as(as = "DisplayFromStr")]
        object_path: ObjectPath
    },
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

    info!("Generated report. path: {:?}", report_file_path);
    
    Ok(())
}