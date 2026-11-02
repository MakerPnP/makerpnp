//! The single source of truth for data received from the planner core.
//!
//! Components do not keep their own copies of views. `ui` methods read from [`ProjectData`], and components with state
//! derived from a view (e.g. table rows) compare [`Cached::revision`] during the update pass and rebuild when it changes.
//!
//! The project marks everything stale after a mutating core event, then fetches only the views that open tabs require
//! (see `ProjectTabKind::required_views`), so handlers do not need to know which views an action affects.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::path::{Path, PathBuf};

use planner_app::Event;
use planner_app::{
    LoadOut, PartStates, PcbOverview, PcbUnitAssignments, PcbView, PhaseOverview, PhasePlacements, PhaseReference,
    Phases, PlacementsList, ProcessDefinition, ProcessReference, ProjectOverview, ProjectPcbOverview, ProjectReport,
    ProjectTreeView, ProjectView,
};

/// A value received from the core.
///
/// The revision only changes when the value actually changes, so re-fetching identical data doesn't disturb dependents.
#[derive(Debug)]
pub struct Cached<T> {
    value: Option<T>,
    revision: u64,
    stale: bool,
}

impl<T> Default for Cached<T> {
    fn default() -> Self {
        Self {
            value: None,
            revision: 0,
            stale: false,
        }
    }
}

impl<T> Cached<T> {
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn needs_fetch(&self) -> bool {
        self.value.is_none() || self.stale
    }

    pub fn mark_stale(&mut self) {
        self.stale = true;
    }
}

impl<T: PartialEq> Cached<T> {
    pub fn set(&mut self, value: T) {
        self.stale = false;
        if self.value.as_ref() != Some(&value) {
            self.value = Some(value);
            self.revision += 1;
        }
    }
}

/// Remembers which revisions of the source data a component last rebuilt its derived state from.
#[derive(Debug)]
pub struct Synced<K>(Option<K>);

impl<K> Default for Synced<K> {
    fn default() -> Self {
        Self(None)
    }
}

impl<K: PartialEq> Synced<K> {
    /// Returns true if `current` differs from the last value given.
    pub fn changed(&mut self, current: K) -> bool {
        if self.0.as_ref() == Some(&current) {
            return false;
        }
        self.0 = Some(current);
        true
    }

    /// Forces the next call to [`Self::changed`] to return true.
    ///
    /// Required after a table applies an edit to its own rows.  If the core rejects the edit the re-fetched view is
    /// identical to the previous one, its revision does not change, and the table would keep showing the rejected value.
    pub fn invalidate(&mut self) {
        self.0 = None;
    }
}

/// Identifies a view that can be fetched from the core.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ViewKey {
    Overview,
    Parts,
    Placements,
    Phases,
    ProjectTree,
    ProjectReport,
    PhaseOverview(PhaseReference),
    PhasePlacements(PhaseReference),
    PhaseLoadOut(PhaseReference),
    ProcessDefinition(ProcessReference),
    ProjectPcbOverview(u16),
    PcbUnitAssignments(u16),
    PcbOverview(PathBuf),
}

impl ViewKey {
    pub fn into_event(self) -> Event {
        match self {
            ViewKey::Overview => Event::RequestOverviewView {},
            ViewKey::Parts => Event::RequestPartStatesView {},
            ViewKey::Placements => Event::RequestPlacementsView {},
            ViewKey::Phases => Event::RequestPhasesView {},
            ViewKey::ProjectTree => Event::RequestProjectTreeView {},
            ViewKey::ProjectReport => Event::RequestProjectReportView {},
            ViewKey::PhaseOverview(phase_reference) => Event::RequestPhaseOverviewView {
                phase_reference,
            },
            ViewKey::PhasePlacements(phase_reference) => Event::RequestPhasePlacementsView {
                phase_reference,
            },
            ViewKey::PhaseLoadOut(phase_reference) => Event::RequestPhaseLoadOutView {
                phase_reference,
            },
            ViewKey::ProcessDefinition(process_reference) => Event::RequestProcessDefinitionView {
                process_reference,
            },
            ViewKey::ProjectPcbOverview(pcb) => Event::RequestProjectPcbOverviewView {
                pcb,
            },
            ViewKey::PcbUnitAssignments(pcb) => Event::RequestPcbUnitAssignmentsView {
                pcb,
            },
            ViewKey::PcbOverview(path) => Event::RequestPcbOverviewView {
                path,
            },
        }
    }
}

#[derive(Debug, Default)]
pub struct ProjectData {
    pub overview: Cached<ProjectOverview>,
    pub parts: Cached<PartStates>,
    pub placements: Cached<PlacementsList>,
    pub phases: Cached<Phases>,
    pub project_tree: Cached<ProjectTreeView>,
    pub report: Cached<ProjectReport>,

    phase_overviews: HashMap<PhaseReference, Cached<PhaseOverview>>,
    phase_placements: HashMap<PhaseReference, Cached<PhasePlacements>>,
    load_outs: HashMap<PhaseReference, Cached<LoadOut>>,
    process_definitions: HashMap<ProcessReference, Cached<ProcessDefinition>>,
    project_pcbs: HashMap<u16, Cached<ProjectPcbOverview>>,
    pcb_unit_assignments: HashMap<u16, Cached<PcbUnitAssignments>>,
    pcb_overviews: HashMap<PathBuf, Cached<PcbOverview>>,

    /// Views that could not be fetched, they will not be retried until the next time everything is marked stale.
    failed: HashSet<ViewKey>,
    /// Views whose failure has been reported to the user, cleared when the view can be fetched again.
    reported: HashSet<ViewKey>,
}

fn value_of<'a, K: Eq + Hash, T>(map: &'a HashMap<K, Cached<T>>, key: &K) -> Option<&'a T> {
    map.get(key).and_then(Cached::get)
}

fn revision_of<K: Eq + Hash, T>(map: &HashMap<K, Cached<T>>, key: &K) -> u64 {
    map.get(key).map_or(0, Cached::revision)
}

fn needs_fetch_in<K: Eq + Hash, T>(map: &HashMap<K, Cached<T>>, key: &K) -> bool {
    map.get(key)
        .is_none_or(Cached::needs_fetch)
}

fn mark_stale_in<K, T>(map: &mut HashMap<K, Cached<T>>) {
    map.values_mut()
        .for_each(Cached::mark_stale);
}

impl ProjectData {
    //
    // reading
    //

    /// The phases, in the order defined by the project, empty until they have been fetched.
    pub fn phase_list(&self) -> &[PhaseOverview] {
        self.phases
            .get()
            .map_or(&[], |phases| phases.phases.as_slice())
    }

    pub fn processes(&self) -> &[ProcessReference] {
        self.overview
            .get()
            .map_or(&[], |overview| overview.processes.as_slice())
    }

    pub fn phase_overview(&self, phase: &PhaseReference) -> Option<&PhaseOverview> {
        value_of(&self.phase_overviews, phase)
    }

    pub fn phase_placements(&self, phase: &PhaseReference) -> Option<&PhasePlacements> {
        value_of(&self.phase_placements, phase)
    }

    pub fn phase_placements_revision(&self, phase: &PhaseReference) -> u64 {
        revision_of(&self.phase_placements, phase)
    }

    pub fn load_out(&self, phase: &PhaseReference) -> Option<&LoadOut> {
        value_of(&self.load_outs, phase)
    }

    pub fn load_out_revision(&self, phase: &PhaseReference) -> u64 {
        revision_of(&self.load_outs, phase)
    }

    pub fn process_definition(&self, process: &ProcessReference) -> Option<&ProcessDefinition> {
        value_of(&self.process_definitions, process)
    }

    pub fn process_definition_revision(&self, process: &ProcessReference) -> u64 {
        revision_of(&self.process_definitions, process)
    }

    pub fn project_pcb(&self, pcb_index: u16) -> Option<&ProjectPcbOverview> {
        value_of(&self.project_pcbs, &pcb_index)
    }

    pub fn pcb_unit_assignments(&self, pcb_index: u16) -> Option<&PcbUnitAssignments> {
        value_of(&self.pcb_unit_assignments, &pcb_index)
    }

    pub fn pcb_unit_assignments_revision(&self, pcb_index: u16) -> u64 {
        revision_of(&self.pcb_unit_assignments, &pcb_index)
    }

    /// The overview of the PCB at the given index in the project.
    pub fn pcb_overview(&self, pcb_index: u16) -> Option<&PcbOverview> {
        let path: &Path = &self.project_pcb(pcb_index)?.pcb_path;
        self.pcb_overviews
            .get(path)
            .and_then(Cached::get)
    }

    pub fn pcb_overview_revision(&self, pcb_index: u16) -> u64 {
        self.project_pcb(pcb_index)
            .map_or(0, |pcb| revision_of(&self.pcb_overviews, &pcb.pcb_path))
    }

    //
    // fetching
    //

    pub fn needs_fetch(&self, key: &ViewKey) -> bool {
        if self.failed.contains(key) {
            return false;
        }

        match key {
            ViewKey::Overview => self.overview.needs_fetch(),
            ViewKey::Parts => self.parts.needs_fetch(),
            ViewKey::Placements => self.placements.needs_fetch(),
            ViewKey::Phases => self.phases.needs_fetch(),
            ViewKey::ProjectTree => self.project_tree.needs_fetch(),
            ViewKey::ProjectReport => self.report.needs_fetch(),
            ViewKey::PhaseOverview(phase) => needs_fetch_in(&self.phase_overviews, phase),
            ViewKey::PhasePlacements(phase) => needs_fetch_in(&self.phase_placements, phase),
            ViewKey::PhaseLoadOut(phase) => needs_fetch_in(&self.load_outs, phase),
            ViewKey::ProcessDefinition(process) => needs_fetch_in(&self.process_definitions, process),
            ViewKey::ProjectPcbOverview(pcb) => needs_fetch_in(&self.project_pcbs, pcb),
            ViewKey::PcbUnitAssignments(pcb) => needs_fetch_in(&self.pcb_unit_assignments, pcb),
            ViewKey::PcbOverview(path) => needs_fetch_in(&self.pcb_overviews, path),
        }
    }

    /// Call when the core's state may have changed.
    pub fn mark_all_stale(&mut self) {
        self.overview.mark_stale();
        self.parts.mark_stale();
        self.placements.mark_stale();
        self.phases.mark_stale();
        self.project_tree.mark_stale();
        self.report.mark_stale();
        mark_stale_in(&mut self.phase_overviews);
        mark_stale_in(&mut self.phase_placements);
        mark_stale_in(&mut self.load_outs);
        mark_stale_in(&mut self.process_definitions);
        mark_stale_in(&mut self.project_pcbs);
        mark_stale_in(&mut self.pcb_unit_assignments);
        mark_stale_in(&mut self.pcb_overviews);

        self.failed.clear();
    }

    /// Re-fetches the report, even though nothing is known to have changed.
    pub fn mark_report_stale(&mut self) {
        self.report.mark_stale();
        self.failed
            .remove(&ViewKey::ProjectReport);
    }

    pub fn record_fetched(&mut self, key: &ViewKey) {
        self.reported.remove(key);
    }

    /// Returns true if the user has not yet been told about this view failing.
    ///
    /// Without this, a view that keeps failing would show the same error after every action.
    #[must_use]
    pub fn record_failed(&mut self, key: ViewKey) -> bool {
        self.failed.insert(key.clone());
        self.reported.insert(key)
    }

    pub fn apply_project_view(&mut self, view: ProjectView) {
        match view {
            ProjectView::Overview(overview) => self.overview.set(overview),
            ProjectView::Parts(parts) => self.parts.set(parts),
            ProjectView::Placements(placements) => self.placements.set(placements),
            ProjectView::Phases(phases) => self.phases.set(phases),
            ProjectView::ProjectTree(project_tree) => self.project_tree.set(project_tree),
            ProjectView::ProjectReport(report) => self.report.set(report),
            ProjectView::PhaseOverview(overview) => self
                .phase_overviews
                .entry(overview.phase_reference.clone())
                .or_default()
                .set(overview),
            ProjectView::PhasePlacements(placements) => self
                .phase_placements
                .entry(placements.phase_reference.clone())
                .or_default()
                .set(placements),
            ProjectView::PhaseLoadOut(load_out) => self
                .load_outs
                .entry(load_out.phase_reference.clone())
                .or_default()
                .set(load_out),
            ProjectView::ProcessDefinition(definition) => self
                .process_definitions
                .entry(definition.reference.clone())
                .or_default()
                .set(definition),
            ProjectView::PcbOverview(pcb) => self
                .project_pcbs
                .entry(pcb.index)
                .or_default()
                .set(pcb),
            ProjectView::PcbUnitAssignments(assignments) => self
                .pcb_unit_assignments
                .entry(assignments.index)
                .or_default()
                .set(assignments),
        }
    }

    pub fn apply_pcb_view(&mut self, view: PcbView) {
        match view {
            PcbView::PcbOverview(overview) => self
                .pcb_overviews
                .entry(overview.path.clone())
                .or_default()
                .set(overview),
            PcbView::PanelSizing(_panel_sizing) => {
                // nothing in a project requires this view
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use planner_app::FileReference;

    use super::*;

    fn project_pcb(index: u16, path: &str) -> ProjectPcbOverview {
        ProjectPcbOverview {
            index,
            pcb_file: FileReference::Absolute(PathBuf::from(path)),
            pcb_path: PathBuf::from(path),
        }
    }

    #[test]
    fn cached_needs_fetch_until_it_has_a_value() {
        let mut cached = Cached::<u32>::default();
        assert!(cached.needs_fetch());

        cached.set(1);
        assert!(!cached.needs_fetch());
        assert_eq!(cached.get(), Some(&1));
    }

    #[test]
    fn cached_revision_only_changes_when_the_value_changes() {
        let mut cached = Cached::<u32>::default();
        assert_eq!(cached.revision(), 0);

        cached.set(1);
        assert_eq!(cached.revision(), 1);

        cached.mark_stale();
        assert!(cached.needs_fetch());

        // same value, no longer stale, dependents are not disturbed.
        cached.set(1);
        assert!(!cached.needs_fetch());
        assert_eq!(cached.revision(), 1);

        cached.set(2);
        assert_eq!(cached.revision(), 2);
    }

    #[test]
    fn synced_reports_changes_and_can_be_invalidated() {
        let mut synced = Synced::<(u64, u64)>::default();

        assert!(synced.changed((0, 0)), "the first call is always a change");
        assert!(!synced.changed((0, 0)));
        assert!(synced.changed((1, 0)));
        assert!(!synced.changed((1, 0)));

        synced.invalidate();
        assert!(synced.changed((1, 0)));
    }

    #[test]
    fn keyed_views_need_fetching_until_received_and_after_being_marked_stale() {
        let mut data = ProjectData::default();
        let key = ViewKey::ProjectPcbOverview(0);
        assert!(data.needs_fetch(&key));

        data.apply_project_view(ProjectView::PcbOverview(project_pcb(0, "a.pcb.json")));
        assert!(!data.needs_fetch(&key));

        data.mark_all_stale();
        assert!(data.needs_fetch(&key));
    }

    #[test]
    fn failed_views_are_not_retried_until_marked_stale() {
        let mut data = ProjectData::default();
        let key = ViewKey::ProjectPcbOverview(0);

        assert!(data.record_failed(key.clone()));
        assert!(!data.needs_fetch(&key));

        data.mark_all_stale();
        assert!(data.needs_fetch(&key));
    }

    #[test]
    fn a_failing_view_is_only_reported_once_until_it_succeeds() {
        let mut data = ProjectData::default();
        let key = ViewKey::ProjectPcbOverview(0);

        assert!(data.record_failed(key.clone()));

        data.mark_all_stale();
        assert!(!data.record_failed(key.clone()), "still failing, already reported");

        data.mark_all_stale();
        data.record_fetched(&key);
        assert!(data.record_failed(key), "failing again after having succeeded");
    }

    #[test]
    fn pcb_overview_is_unavailable_until_the_project_pcb_link_is_known() {
        let mut data = ProjectData::default();
        assert_eq!(data.pcb_overview_revision(0), 0);
        assert!(data.pcb_overview(0).is_none());

        data.apply_project_view(ProjectView::PcbOverview(project_pcb(0, "a.pcb.json")));
        assert!(data.pcb_overview(0).is_none());
        assert!(data.needs_fetch(&ViewKey::PcbOverview(PathBuf::from("a.pcb.json"))));
    }
}
