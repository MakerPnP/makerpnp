use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::io::Write;
use std::path::{Path, PathBuf};

use dyn_clone::DynClone;
#[cfg(feature = "markdown")]
use json2markdown::MarkdownRenderer;
use pnp::load_out::LoadOutItem;
use pnp::object_path::ObjectPath;
use pnp::part::Part;
use pnp::reference::Reference;
use serde::Serialize;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use tracing::{error, info, trace};
use util::dynamic::dynamic_eq::DynamicEq;
use util::sorting::SortOrder;

use crate::design::{DesignName, DesignVariant};
use crate::file::FileReference;
use crate::pcb::Pcb;
use crate::phase::{PhaseReference, PhaseStatus};
use crate::placement::{PlacementState, ProjectPlacementStatus};
use crate::process::{OperationReference, OperationStatus, TaskKind, TaskReference};
use crate::project::{build_phase_placement_states, Project};
use crate::variant::VariantName;

// FUTURE add a test to ensure that duplicate issues are not added to the report.
//        currently a BTreeSet is used to prevent duplicate issues.

pub fn project_generate_report(
    project: &Project,
    pcbs: &[&Pcb],
    phase_load_out_items_map: &BTreeMap<PhaseReference, Vec<LoadOutItem>>,
) -> ProjectReport {
    let mut report = ProjectReport::default();

    let mut issue_set: BTreeSet<ProjectReportIssue> = BTreeSet::new();

    report.name.clone_from(&project.name);
    if project.pcbs.is_empty() {
        issue_set.insert(ProjectReportIssue {
            message: "No PCBs have been assigned to the project.".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPcbsAssigned,
        });
    } else {
        for pcb in project.pcbs.iter() {
            if pcb.unit_assignments.is_empty() {
                issue_set.insert(ProjectReportIssue {
                    message: "A PCB has no unit assignments.".to_string(),
                    severity: IssueSeverity::Severe,
                    kind: IssueKind::PcbWithNoUnitAssignments {
                        file: pcb.pcb_file.clone(),
                    },
                });
            }
        }
    }

    if project.placements.is_empty() {
        issue_set.insert(ProjectReportIssue {
            message: "No placements.".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPlacements,
        });
    }

    for phase_reference in project.phase_orderings.iter() {
        let phase_placement_states = build_phase_placement_states(project, phase_reference);
        if phase_placement_states.is_empty() {
            issue_set.insert(ProjectReportIssue {
                message: "Phase with no placements.".to_string(),
                severity: IssueSeverity::Warning,
                kind: IssueKind::PhaseWithNoPlacements {
                    phase: phase_reference.clone(),
                },
            });
        }

        for (_object_path, placement_state) in phase_placement_states.iter() {
            let load_out_items = phase_load_out_items_map
                .get(phase_reference)
                .unwrap();

            let feeder_reference =
                match pnp::load_out::find_load_out_item_by_part(load_out_items, &placement_state.placement.part) {
                    Some(load_out_item) => load_out_item.reference.clone(),
                    _ => None,
                };

            if feeder_reference.is_none() {
                let issue = ProjectReportIssue {
                    message: "A part has not been assigned to a feeder".to_string(),
                    severity: IssueSeverity::Warning,
                    kind: IssueKind::UnassignedPartFeeder {
                        phase: phase_reference.clone(),
                        part: placement_state.placement.part.clone(),
                    },
                };
                issue_set.insert(issue);
            };
        }
    }

    let mut all_phases_complete = true;

    if !project.phases.is_empty() {
        report.phase_overviews.extend(
            project
                .phase_orderings
                .iter()
                .map(|reference| {
                    let phase = project.phases.get(reference).unwrap();
                    let phase_state = project
                        .phase_states
                        .get(reference)
                        .unwrap();
                    trace!("phase: {:?}, phase_state: {:?}", phase, phase_state);

                    let operations_overview = phase_state
                        .operation_states
                        .iter()
                        .map(|operation_state| {
                            let task_overviews = operation_state
                                .task_states
                                .iter()
                                .filter_map(|(task_reference, task_state)| {
                                    let kind =
                                        project.find_task_kind(reference, &operation_state.reference, task_reference);
                                    let report = match kind {
                                        Some(TaskKind::Generic) => {
                                            Some(Box::new(GenericTaskOverview {}) as Box<dyn TaskOverview>)
                                        }
                                        Some(TaskKind::LoadPcbs) => {
                                            Some(Box::new(LoadPcbsTaskOverview {}) as Box<dyn TaskOverview>)
                                        }
                                        Some(TaskKind::PlaceComponents) => task_state
                                            .placements_state()
                                            .map(|state| {
                                                let summary = state.summary();
                                                Box::new(PlaceComponentsTaskOverview {
                                                    placed: summary.placed,
                                                    skipped: summary.skipped,
                                                    total: summary.total,
                                                })
                                                    as Box<dyn TaskOverview>
                                            }),
                                        None => None,
                                    };
                                    Some((task_reference.clone(), report))
                                })
                                .collect::<Vec<_>>();

                            let operation_status = operation_state.status();

                            let overview = PhaseOperationOverview {
                                operation: operation_state.reference.clone(),
                                status: operation_status,
                                tasks: task_overviews,
                            };

                            overview
                        })
                        .collect::<Vec<_>>();

                    let phase_status = phase_state.status();

                    if phase_status != PhaseStatus::Complete {
                        all_phases_complete = false
                    }

                    PhaseOverview {
                        phase: phase.reference.clone(),
                        status: phase_status,
                        process: phase.process.to_string(),
                        operations_overview,
                    }
                }),
        );
    } else {
        issue_set.insert(ProjectReportIssue {
            message: "No phases have been created.".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::NoPhasesCreated,
        });
    }

    report.status = match all_phases_complete {
        true => ProjectStatus::Complete,
        false => ProjectStatus::Incomplete,
    };

    let phase_specifications: Vec<PhaseSpecification> = project
        .phase_orderings
        .iter()
        .map(|reference| build_phase_specification(project, pcbs, phase_load_out_items_map, reference))
        .collect();

    report
        .phase_specifications
        .extend(phase_specifications);

    project_report_add_placement_issues(project, &mut issue_set);
    let mut issues: Vec<ProjectReportIssue> = issue_set.iter().cloned().collect();

    project_report_sort_issues(&mut issues);

    for issue in issues.iter() {
        info!(
            "Issue detected. severity: {:?}, message: '{}', kind: {:?}",
            issue.severity, issue.message, issue.kind
        );
    }

    report.issues = issues;

    report
}

fn build_phase_specification(
    project: &Project,
    pcbs: &[&Pcb],
    phase_load_out_items_map: &BTreeMap<Reference, Vec<LoadOutItem>>,
    reference: &Reference,
) -> PhaseSpecification {
    let phase = project.phases.get(reference).unwrap();
    let phase_state = project
        .phase_states
        .get(reference)
        .unwrap();

    let load_out_items = phase_load_out_items_map
        .get(reference)
        .unwrap();

    let load_out_assignments = load_out_items.iter().map(|load_out_item| {
        let quantity = project.placements.iter()
            .filter(|(_object_path, placement_state)| {
                matches!(&placement_state.phase, Some(other_phase_reference) if phase.reference.eq(other_phase_reference))
                    && placement_state.placement.place
                    && load_out_item.manufacturer.eq(&placement_state.placement.part.manufacturer)
                    && load_out_item.mpn.eq(&placement_state.placement.part.mpn)
            })
            .fold(0_u32, |quantity, _placement_state| {
                quantity + 1
            });

        PhaseLoadOutAssignmentItem {
            feeder_reference: load_out_item.reference.clone(),
            manufacturer: load_out_item.manufacturer.clone(),
            mpn: load_out_item.mpn.clone(),
            quantity,
        }
    }).collect();

    let operations = phase_state
        .operation_states
        .iter()
        .map(|operation_state| {
            let task_specifications = operation_state
                .task_states
                .iter()
                .filter_map(|(task_reference, _task_state)| {
                    let kind = project.find_task_kind(reference, &operation_state.reference, task_reference);
                    let report = match kind {
                        Some(TaskKind::Generic) => {
                            Some(Box::new(GenericTaskSpecification {}) as Box<dyn TaskSpecification>)
                        }
                        Some(TaskKind::LoadPcbs) => {
                            let pcbs = build_operation_load_pcbs(project, pcbs);
                            Some(Box::new(LoadPcbsTaskSpecification {
                                pcbs,
                            }) as Box<dyn TaskSpecification>)
                        }
                        Some(TaskKind::PlaceComponents) => {
                            Some(Box::new(PlaceComponentsTaskSpecification {}) as Box<dyn TaskSpecification>)
                        }
                        None => None,
                    };
                    Some((task_reference.clone(), report))
                })
                .collect::<Vec<_>>();

            let operation_item = OperationItem {
                operation: operation_state.reference.clone(),
                task_specifications,
            };

            operation_item
        })
        .collect();

    PhaseSpecification {
        phase: phase.reference.clone(),
        operations,
        load_out_assignments,
    }
}

fn build_operation_load_pcbs(project: &Project, pcbs: &[&Pcb]) -> Vec<PcbReportItem> {
    let unit_paths_with_placements = build_unit_paths_with_placements(&project.placements);

    let pcbs: Vec<PcbReportItem> = unit_paths_with_placements
        .iter()
        .find_map(|unit_path| {
            if let Ok(pcb_instance) = unit_path.pcb_instance() {
                let pcb_index = (pcb_instance - 1) as usize;

                // Note: the user may not have made any unit assignments yet.
                let unit_assignments = find_unit_assignments(project, pcbs, unit_path);

                let (_project_pcb, pcb) = (project.pcbs.get(pcb_index).unwrap(), pcbs.get(pcb_index).unwrap());

                Some(PcbReportItem {
                    name: pcb.name.clone(),
                    unit_assignments,
                })
            } else {
                None
            }
        })
        .into_iter()
        .collect();

    pcbs
}

fn build_unit_paths_with_placements(placement_states: &BTreeMap<ObjectPath, PlacementState>) -> BTreeSet<ObjectPath> {
    placement_states.iter().fold(
        BTreeSet::<ObjectPath>::new(),
        |mut acc, (object_path, placement_state)| {
            if placement_state.placement.place {
                if let Ok(pcb_unit) = object_path.pcb_unit_path() {
                    if acc.insert(pcb_unit) {
                        trace!("Phase pcb unit found.  object_path: {}", object_path);
                    }
                } else {
                    error!("pcb unit not specified.  object_path: {}", object_path);
                }
            }
            acc
        },
    )
}

fn project_report_add_placement_issues(project: &Project, issues: &mut BTreeSet<ProjectReportIssue>) {
    for (object_path, _placement_state) in project
        .placements
        .iter()
        .filter(|(_object_path, placement_state)| {
            placement_state.phase.is_none() && placement_state.project_status == ProjectPlacementStatus::Used
        })
    {
        issues.insert(ProjectReportIssue {
            message: "A placement has not been assigned to a phase".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::UnassignedPlacement {
                object_path: object_path.clone(),
            },
        });
    }
}

fn project_report_sort_issues(issues: &mut [ProjectReportIssue]) {
    issues.sort_by(|a, b| {
        let sort_orderings = &[
            ("severity", SortOrder::Desc),
            ("kind", SortOrder::Asc),
            ("message", SortOrder::Asc),
        ];

        sort_orderings
            .iter()
            .fold(Ordering::Equal, |mut acc, (&ref mode, sort_order)| {
                if !matches!(acc, Ordering::Equal) {
                    return acc;
                }

                fn kind_ordinal(kind: &IssueKind) -> usize {
                    match kind {
                        IssueKind::NoPcbsAssigned => 0,
                        IssueKind::NoPhasesCreated => 1,
                        IssueKind::PcbWithNoUnitAssignments {
                            ..
                        } => 2,
                        IssueKind::NoPlacements => 3,
                        IssueKind::PhaseWithNoPlacements {
                            ..
                        } => 4,
                        IssueKind::UnassignedPlacement {
                            ..
                        } => 5,
                        IssueKind::UnassignedPartFeeder {
                            ..
                        } => 6,
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
                            Ordering::Equal => a.kind.cmp(&b.kind),
                            Ordering::Greater => ordinal_ordering,
                        }
                    }
                    "message" => a.message.cmp(&b.message),
                    "severity" => {
                        let a_ordinal = severity_ordinal(&a.severity);
                        let b_ordinal = severity_ordinal(&b.severity);
                        let ordinal_ordering = a_ordinal.cmp(&b_ordinal);
                        ordinal_ordering
                    }
                    _ => unreachable!(),
                };

                match sort_order {
                    SortOrder::Asc => acc,
                    SortOrder::Desc => acc.reverse(),
                }
            })
    });
}

#[cfg(test)]
mod report_issue_sorting {
    use std::str::FromStr;

    use pnp::object_path::ObjectPath;
    use pnp::part::Part;

    use crate::phase::PhaseReference;
    use crate::report::{project_report_sort_issues, IssueKind, IssueSeverity, ProjectReportIssue};

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

        let mut issues: Vec<ProjectReportIssue> = vec![issue2.clone(), issue1.clone()];
        let expected_issues: Vec<ProjectReportIssue> = vec![issue1.clone(), issue2.clone()];

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

        let mut issues: Vec<ProjectReportIssue> = vec![issue2.clone(), issue1.clone()];
        let expected_issues: Vec<ProjectReportIssue> = vec![issue1.clone(), issue2.clone()];

        // when
        project_report_sort_issues(&mut issues);

        // then
        assert_eq!(&issues, &expected_issues);
    }

    #[test]
    pub fn sort_by_phase() {
        // given
        let part = Part {
            manufacturer: "MFR1".to_string(),
            mpn: "MPN1".to_string(),
        };

        let issue1 = ProjectReportIssue {
            message: "MESSAGE_1".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::UnassignedPartFeeder {
                phase: PhaseReference::from_raw_str("phase_1"),
                part: part.clone(),
            },
        };
        let issue2 = ProjectReportIssue {
            message: "MESSAGE_1".to_string(),
            severity: IssueSeverity::Warning,
            kind: IssueKind::UnassignedPartFeeder {
                phase: PhaseReference::from_raw_str("phase_2"),
                part: part.clone(),
            },
        };

        let mut issues: Vec<ProjectReportIssue> = vec![issue2.clone(), issue1.clone()];
        let expected_issues: Vec<ProjectReportIssue> = vec![issue1.clone(), issue2.clone()];

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
            kind: IssueKind::UnassignedPlacement {
                object_path: ObjectPath::from_str("pcb=1::unit=1::ref_des=R1").expect("always ok"),
            },
        };
        let issue4 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPlacement {
                object_path: ObjectPath::from_str("pcb=1::unit=1::ref_des=R2").expect("always ok"),
            },
        };
        let issue5 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPartFeeder {
                phase: PhaseReference::from_raw_str("phase_1"),
                part: Part {
                    manufacturer: "MFR1".to_string(),
                    mpn: "MPN1".to_string(),
                },
            },
        };
        let issue6 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPartFeeder {
                phase: PhaseReference::from_raw_str("phase_1"),
                part: Part {
                    manufacturer: "MFR1".to_string(),
                    mpn: "MPN2".to_string(),
                },
            },
        };
        let issue7 = ProjectReportIssue {
            message: "EQUAL".to_string(),
            severity: IssueSeverity::Severe,
            kind: IssueKind::UnassignedPartFeeder {
                phase: PhaseReference::from_raw_str("phase_1"),
                part: Part {
                    manufacturer: "MFR2".to_string(),
                    mpn: "MPN1".to_string(),
                },
            },
        };

        let mut issues: Vec<ProjectReportIssue> = vec![
            issue7.clone(),
            issue6.clone(),
            issue5.clone(),
            issue4.clone(),
            issue3.clone(),
            issue2.clone(),
            issue1.clone(),
        ];
        let expected_issues: Vec<ProjectReportIssue> = vec![
            issue1.clone(),
            issue2.clone(),
            issue3.clone(),
            issue4.clone(),
            issue5.clone(),
            issue6.clone(),
            issue7.clone(),
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
            issue1.clone(),
            issue2.clone(),
            issue3.clone(),
            issue4.clone(),
            issue5.clone(),
            issue6.clone(),
        ];
        let expected_issues: Vec<ProjectReportIssue> = vec![
            issue1.clone(),
            issue4.clone(),
            issue3.clone(),
            issue2.clone(),
            issue6.clone(),
            issue5.clone(),
        ];

        // when
        project_report_sort_issues(&mut issues);

        // then
        assert_eq!(&issues, &expected_issues);
    }
}

fn find_unit_assignments(project: &Project, pcbs: &[&Pcb], unit_path: &ObjectPath) -> Vec<PcbUnitAssignmentItem> {
    let all_unit_assignments = project.all_unit_assignments(pcbs);

    let unit_assignments = all_unit_assignments
        .iter()
        .filter_map(|(assignment_unit_path, unit_assignment)| {
            let mut result = None;

            if let Some(DesignVariant {
                design_name,
                variant_name,
            }) = unit_assignment
            {
                if assignment_unit_path.eq(unit_path) {
                    result = Some(PcbUnitAssignmentItem {
                        unit_path: unit_path.clone(),
                        design_name: design_name.clone(),
                        variant_name: variant_name.clone(),
                    })
                }
            }
            result
        })
        .collect();

    unit_assignments
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Default, Debug, PartialEq)]
pub struct ProjectReport {
    pub name: String,
    pub status: ProjectStatus,
    pub phase_overviews: Vec<PhaseOverview>,
    pub phase_specifications: Vec<PhaseSpecification>,
    /// A list of unique issues.
    /// Note: Using a Vec doesn't prevent duplicates, duplicates must be filtered before adding them.
    pub issues: Vec<ProjectReportIssue>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ProjectStatus {
    Incomplete,
    Complete,
}

impl Default for ProjectStatus {
    fn default() -> Self {
        Self::Incomplete
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PhaseOverview {
    pub phase: PhaseReference,
    pub status: PhaseStatus,
    pub process: String,
    pub operations_overview: Vec<PhaseOperationOverview>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PhaseSpecification {
    pub phase: PhaseReference,
    pub operations: Vec<OperationItem>,
    pub load_out_assignments: Vec<PhaseLoadOutAssignmentItem>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PhaseOperationOverview {
    pub operation: OperationReference,
    pub status: OperationStatus,
    pub tasks: Vec<(TaskReference, Option<Box<dyn TaskOverview>>)>,
}

impl PartialEq for PhaseOperationOverview {
    fn eq(&self, other: &Self) -> bool {
        if self.operation != other.operation {
            return false;
        }

        if self.status != other.status {
            return false;
        }

        for ((a_ref, a_spec), (b_ref, b_spec)) in self
            .tasks
            .iter()
            .zip(other.tasks.iter())
        {
            if a_ref != b_ref {
                return false;
            }

            match (a_spec, b_spec) {
                (Some(a_spec), Some(b_spec)) => {
                    if !a_spec.dynamic_eq(b_spec) {
                        return false;
                    }
                }
                _ => return false,
            }
        }

        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OperationItem {
    pub operation: OperationReference,
    pub task_specifications: Vec<(TaskReference, Option<Box<dyn TaskSpecification>>)>,
}

impl PartialEq for OperationItem {
    fn eq(&self, other: &Self) -> bool {
        if self.operation != other.operation {
            return false;
        }

        if self.task_specifications.len() != other.task_specifications.len() {
            return false;
        }

        for ((a_ref, a_spec), (b_ref, b_spec)) in self
            .task_specifications
            .iter()
            .zip(other.task_specifications.iter())
        {
            if a_ref != b_ref {
                return false;
            }

            match (a_spec, b_spec) {
                (Some(a_spec), Some(b_spec)) => {
                    if !a_spec.dynamic_eq(b_spec) {
                        return false;
                    }
                }
                _ => return false,
            }
        }

        true
    }
}

#[typetag::serde(tag = "type")]
pub trait TaskSpecification: DynClone + DynamicEq + Debug + Send {}
dyn_clone::clone_trait_object!(TaskSpecification);

macro_rules! generic_task_specification {
    ($name:ident, $key:literal) => {
        #[typetag::serde(name = $key)]
        impl TaskSpecification for $name {}

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
        pub struct $name {}
    };
}

generic_task_specification!(GenericTaskSpecification, "generic_specification");
generic_task_specification!(PlaceComponentsTaskSpecification, "place_components_specification");

#[typetag::serde(name = "load_pcbs_specification")]
impl TaskSpecification for LoadPcbsTaskSpecification {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
struct LoadPcbsTaskSpecification {
    pub pcbs: Vec<PcbReportItem>,
}

#[typetag::serde(tag = "type")]
pub trait TaskOverview: DynClone + DynamicEq + Debug + Send {}
dyn_clone::clone_trait_object!(TaskOverview);

macro_rules! generic_task_overview {
    ($name:ident, $key:literal) => {
        #[typetag::serde(name = $key)]
        impl TaskOverview for $name {}

        #[derive(Clone, serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        pub struct $name {}
    };
}

generic_task_overview!(LoadPcbsTaskOverview, "load_pcbs_overview");
generic_task_overview!(GenericTaskOverview, "generic_overview");

#[typetag::serde(name = "place_components_overview")]
impl TaskOverview for PlaceComponentsTaskOverview {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PlaceComponentsTaskOverview {
    pub placed: usize,
    pub skipped: usize,
    pub total: usize,
}

#[serde_as]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PcbUnitAssignmentItem {
    #[serde_as(as = "DisplayFromStr")]
    unit_path: ObjectPath,
    design_name: DesignName,
    variant_name: VariantName,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PcbReportItem {
    name: String,
    unit_assignments: Vec<PcbUnitAssignmentItem>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PhaseLoadOutAssignmentItem {
    pub feeder_reference: Option<Reference>,
    pub manufacturer: String,
    pub mpn: String,
    pub quantity: u32,
}

// FUTURE implement `Display` and improve info logging
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectReportIssue {
    pub message: String,
    pub severity: IssueSeverity,
    pub kind: IssueKind,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    Severe,
    Warning,
}

#[serde_as]
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueKind {
    NoPcbsAssigned,
    NoPhasesCreated,
    UnassignedPlacement {
        #[serde_as(as = "DisplayFromStr")]
        object_path: ObjectPath,
    },
    UnassignedPartFeeder {
        phase: PhaseReference,
        part: Part,
    },
    PcbWithNoUnitAssignments {
        file: FileReference,
    },
    NoPlacements,
    PhaseWithNoPlacements {
        phase: PhaseReference,
    },
}

pub(crate) fn build_report_file_path(name: &str, directory: &Path) -> PathBuf {
    let mut report_file_path: PathBuf = PathBuf::from(directory);
    report_file_path.push(format!("{}_report.json", name));
    report_file_path
}

// HACK: core-io-hack - the report is written by the core, until the stores service writes it.
#[allow(clippy::disallowed_methods, clippy::disallowed_types)]
pub(crate) fn project_report_save_as_json(report: &ProjectReport, report_file_path: &PathBuf) -> anyhow::Result<()> {
    let report_file = std::fs::File::create(report_file_path)?;
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(report_file, formatter);
    report.serialize(&mut ser)?;

    let mut report_file = ser.into_inner();
    let _written = report_file.write(b"\n")?;

    info!("Generated JSON report. path: {:?}", report_file_path);

    Ok(())
}

#[cfg(feature = "markdown")]
// HACK: core-io-hack - the markdown report is read and written by the core, until the stores service does it.
#[allow(clippy::disallowed_methods)]
pub fn project_report_json_to_markdown(json_report_file_name: &PathBuf) -> anyhow::Result<()> {
    let json_string = std::fs::read_to_string(json_report_file_name)?;

    let json = serde_json::from_str(&json_string)?;

    let renderer = MarkdownRenderer::default();
    let markdown = renderer.render(&json);

    /// Replace the file extension with a different one.
    ///
    /// * New extension should not start with a '.'.
    /// * If there's no existing extension, it adds the new one
    /// * If there is an extension, it replaces it.
    fn replace_extension(mut path: PathBuf, extension: &str) -> PathBuf {
        assert!(!extension.starts_with('.'));
        if let Some(file_name) = path
            .file_name()
            .and_then(|n| n.to_str())
        {
            if let Some(new_name) = Path::new(file_name)
                .with_extension(extension)
                .to_str()
            {
                path.set_file_name(new_name);
            }
        }
        path
    }

    let markdown_file_name = replace_extension(json_report_file_name.clone(), "md");
    std::fs::write(&markdown_file_name, markdown)?;

    info!("Generated Markdown report. path: {:?}", markdown_file_name);

    Ok(())
}
