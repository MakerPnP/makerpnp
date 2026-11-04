use dyn_clone::DynClone;
use serde::Serialize;

use crate::common::project_builder::TestProcessOperationStatus;

#[derive(Default)]
pub struct ProjectReportBuilder {
    report: TestProjectReport,
}

impl ProjectReportBuilder {
    pub fn with_phase_specification(mut self, phase_specifications: &[TestPhaseSpecification]) -> Self {
        self.report.phase_specifications = Some(Vec::from(phase_specifications));
        self
    }

    pub fn with_issues(mut self, issues: &[TestIssue]) -> Self {
        self.report.issues = Some(Vec::from(issues));
        self
    }

    pub fn with_phases_overview(mut self, phase_overviews: &[TestPhaseOverview]) -> Self {
        self.report.phase_overviews = Some(Vec::from(phase_overviews));
        self
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.report.name = Some(name.to_string());
        self
    }

    pub fn with_status(mut self, status: &str) -> Self {
        self.report.status = Some(status.to_string());
        self
    }

    pub fn as_string(&mut self) -> String {
        let mut buffer = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut ser = serde_json::Serializer::with_formatter(&mut buffer, formatter);

        self.report
            .serialize(&mut ser)
            .expect("ok");

        let mut content = String::from_utf8(buffer).unwrap();
        content.push('\n');

        content
    }
}

#[derive(Clone, serde::Serialize, Default)]
pub struct TestProjectReport {
    name: Option<String>,
    status: Option<String>,
    phase_overviews: Option<Vec<TestPhaseOverview>>,
    phase_specifications: Option<Vec<TestPhaseSpecification>>,
    issues: Option<Vec<TestIssue>>,
}

#[derive(Clone, serde::Serialize)]
pub struct TestPhaseOverview {
    pub phase: String,
    pub status: String,
    pub process: String,
    pub operations_overview: Vec<TestPhaseOperationOverview>,
}

#[derive(Clone, serde::Serialize)]
pub struct TestPhaseOperationOverview {
    pub operation: String,
    pub status: TestProcessOperationStatus,
    pub tasks: Vec<(String, Box<dyn TestTaskOverview>)>,
}

#[typetag::serialize(tag = "type")]
pub trait TestTaskOverview: DynClone {}
dyn_clone::clone_trait_object!(TestTaskOverview);

macro_rules! generic_test_task_overview {
    ($name:ident, $key:literal) => {
        #[typetag::serialize(name = $key)]
        impl TestTaskOverview for $name {}

        #[derive(Clone, serde::Serialize)]
        pub struct $name {}
    };
}

generic_test_task_overview!(TestLoadPcbsTaskOverview, "load_pcbs_overview");
generic_test_task_overview!(TestGenericTaskOverview, "generic_overview");

#[typetag::serialize(name = "place_components_overview")]
impl TestTaskOverview for TestPlaceComponentsTaskOverview {}

#[derive(Clone, serde::Serialize)]
pub struct TestPlaceComponentsTaskOverview {
    pub placed: usize,
    pub skipped: usize,
    pub total: usize,
}

#[derive(Clone, serde::Serialize)]
pub struct TestPhaseSpecification {
    pub phase: String,
    pub operations: Vec<TestPhaseOperation>,
    pub load_out_assignments: Vec<TestPhaseLoadOutAssignmentItem>,
}

#[derive(Clone, serde::Serialize)]
pub struct TestPhaseOperation {
    pub operation: String,
    pub task_specifications: Vec<(String, Box<dyn TestTaskSpecification>)>,
}

#[typetag::serialize(tag = "type")]
pub trait TestTaskSpecification: DynClone {}
dyn_clone::clone_trait_object!(TestTaskSpecification);

macro_rules! generic_test_task_specification {
    ($name:ident, $key:literal) => {
        #[typetag::serialize(name = $key)]
        impl TestTaskSpecification for $name {}

        #[derive(Clone, serde::Serialize)]
        pub struct $name {}
    };
}

generic_test_task_specification!(TestGenericTaskSpecification, "generic_specification");
generic_test_task_specification!(TestPlaceComponentsTaskSpecification, "place_components_specification");

#[typetag::serialize(name = "load_pcbs_specification")]
impl TestTaskSpecification for TestLoadPcbsTaskSpecification {}

#[derive(Clone, serde::Serialize)]
pub struct TestLoadPcbsTaskSpecification {
    pub pcbs: Vec<TestPcb>,
}

#[derive(Clone, serde::Serialize)]
pub struct TestPcb {
    pub name: String,
    pub unit_assignments: Vec<TestPcbUnitAssignment>,
}

#[derive(Clone, serde::Serialize)]
pub struct TestPcbUnitAssignment {
    pub unit_path: String,
    pub design_name: String,
    pub variant_name: String,
}

#[derive(Clone, serde::Serialize)]
pub struct TestPhaseLoadOutAssignmentItem {
    pub feeder_reference: Option<String>,
    pub manufacturer: String,
    pub mpn: String,
    pub quantity: u32,
    // FUTURE maybe add list of object paths?
}

#[derive(Clone, serde::Serialize)]
pub enum TestIssueSeverity {
    Warning,
}

#[derive(Clone, serde::Serialize)]
pub enum TestIssueKind {
    UnassignedPlacement { object_path: String },
    UnassignedPartFeeder { phase: String, part: TestPart },
    PhaseWithNoPlacements { phase: String },
}

#[derive(Clone, serde::Serialize)]
pub struct TestPart {
    pub manufacturer: String,
    pub mpn: String,
}

#[derive(Clone, serde::Serialize)]
pub struct TestIssue {
    pub message: String,
    pub severity: TestIssueSeverity,
    pub kind: TestIssueKind,
}
