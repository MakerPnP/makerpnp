use serde::Serialize;

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

        self.report.serialize(&mut ser).expect("ok");

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
    pub phase_name: String,
    pub status: String,
    pub process: String,
    pub operations_overview: Vec<TestPhaseOperationOverview>,
}

#[derive(Clone, serde::Serialize)]
pub struct TestPhaseOperationOverview {
    pub operation: TestPhaseOperationKind,
    pub message: String,
    pub complete: bool
}

#[derive(Clone, serde::Serialize)]
pub enum TestPhaseOperationKind {
    PlaceComponents
}

#[derive(Clone, serde::Serialize)]
pub struct TestPhaseSpecification {
    pub phase_name: String,
    pub operations: Vec<TestPhaseOperation>,
    pub load_out_assignments: Vec<TestPhaseLoadOutAssignmentItem>
}

#[derive(Clone, serde::Serialize)]
pub enum TestPhaseOperation {
    PreparePcbs { pcbs: Vec<TestPcb> },
    PlaceComponents {},
    ManuallySolderComponents {},
    ReflowComponents {},
    // FUTURE add `LoadFeeders {...}`
}

#[derive(Clone, serde::Serialize)]
pub enum TestPcb {
    Single { name: String, unit_assignment: TestPcbUnitAssignment },
    Panel { name: String, unit_assignments: Vec<TestPcbUnitAssignment> },
}

#[derive(Clone, serde::Serialize)]
pub struct TestPcbUnitAssignment {
    pub unit_path: String,
    pub design_name: String,
    pub variant_name: String,
}

#[derive(Clone, serde::Serialize)]
pub struct TestPhaseLoadOutAssignmentItem {
    pub feeder_reference: String, 
    pub manufacturer: String, 
    pub mpn: String,
    pub quantity: u32,
    // FUTURE maybe add list of object paths?
}

#[derive(Clone, serde::Serialize)]
pub enum TestIssueSeverity {
    Warning
}

#[derive(Clone, serde::Serialize)]
pub enum TestIssueKind {
    UnassignedPlacement { object_path: String },
    UnassignedPartFeeder { part: TestPart },
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
