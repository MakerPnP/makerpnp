use serde::Serialize;

#[derive(Default)]
pub struct ProjectReportBuilder {
    report: TestProjectReport,
}

impl ProjectReportBuilder {
    pub fn with_phases_overview(mut self, phase_overviews: &[TestPhaseOverview]) -> Self {
        self.report.phase_overviews = Some(Vec::from(phase_overviews));
        self
    }
}

impl ProjectReportBuilder {
    pub fn with_name(mut self, name: &str) -> Self {
        self.report.name = Some(name.to_string());
        self
    }
}

impl ProjectReportBuilder {
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
    phase_overviews: Option<Vec<TestPhaseOverview>>,
}

#[derive(Clone, serde::Serialize)]
pub struct TestPhaseOverview {
    pub phase_name: String,
}