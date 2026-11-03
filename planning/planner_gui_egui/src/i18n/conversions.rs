use planner_app::{
    GerberFileFunction, GerberFileFunctionDiscriminants, OperationStatus, PcbAssemblyFlip, PcbSide, PhaseStatus,
    PlacementSortingMode, PlacementStatus, ProjectPlacementStatus, TaskAction, TaskStatus,
};
use util::sorting::SortOrder;

pub fn pcb_side_to_i18n_key(pcb_side: &PcbSide) -> &'static str {
    match pcb_side {
        PcbSide::Top => "pcb-side-top",
        PcbSide::Bottom => "pcb-side-bottom",
    }
}

pub fn placement_operation_status_to_i18n_key(placement_operation_status: &PlacementStatus) -> &'static str {
    match placement_operation_status {
        PlacementStatus::Placed => "placement-placed",
        PlacementStatus::Skipped => "placement-skipped",
        PlacementStatus::Pending => "placement-pending",
    }
}

pub fn placement_place_to_i18n_key(placed: bool) -> &'static str {
    match placed {
        true => "placement-place",
        false => "placement-no-place",
    }
}

pub fn placement_project_status_to_i18n_key(status: &ProjectPlacementStatus) -> &'static str {
    match status {
        ProjectPlacementStatus::Used => "placement-project-status-used",
        ProjectPlacementStatus::Unused => "placement-project-status-unused",
    }
}

pub fn placement_sorting_mode_to_i18n_key(mode: &PlacementSortingMode) -> &'static str {
    match mode {
        PlacementSortingMode::FeederReference => "sort-mode-feeder-reference",
        PlacementSortingMode::Pcb => "sort-mode-pcb",
        PlacementSortingMode::PcbUnit => "sort-mode-pcb-unit",
        PlacementSortingMode::PcbUnitXY => "sort-mode-pcb-unit-xy",
        PlacementSortingMode::PcbUnitYX => "sort-mode-pcb-unit-yx",
        PlacementSortingMode::RefDes => "sort-mode-ref-des",
        PlacementSortingMode::Area => "sort-mode-area",
        PlacementSortingMode::Height => "sort-mode-height",
        PlacementSortingMode::Part => "sort-mode-part",
    }
}

pub fn sort_order_to_i18n_key(sort_order: &SortOrder) -> &'static str {
    match sort_order {
        SortOrder::Asc => "sort-order-ascending",
        SortOrder::Desc => "sort-order-descending",
    }
}

/// Note: uses the same i18n keys for [`TaskStatus`]
pub fn process_operation_status_to_i18n_key(status: &OperationStatus) -> &'static str {
    match status {
        OperationStatus::Pending => "process-status-pending",
        OperationStatus::Started => "process-status-incomplete",
        OperationStatus::Complete => "process-status-complete",
        OperationStatus::Abandoned => "process-status-abandoned",
    }
}

/// Note: uses the same i18n keys for [`OperationStatus`]
pub fn process_task_status_to_i18n_key(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "process-status-pending",
        TaskStatus::Started => "process-status-incomplete",
        TaskStatus::Complete => "process-status-complete",
        TaskStatus::Abandoned => "process-status-abandoned",
    }
}

pub fn process_task_action_to_i18n_key(action: &TaskAction) -> &'static str {
    match action {
        TaskAction::Start => "process-task-action-start",
        TaskAction::Complete => "process-task-action-complete",
        TaskAction::Abandon => "process-task-action-abandon",
    }
}

pub fn phase_status_to_i18n_key(status: &PhaseStatus) -> &'static str {
    match status {
        PhaseStatus::Pending => "process-status-pending",
        PhaseStatus::Incomplete => "process-status-incomplete",
        PhaseStatus::Complete => "process-status-complete",
        PhaseStatus::Abandoned => "process-status-abandoned",
    }
}

pub fn gerber_file_function_to_i18n_key(function: &GerberFileFunction) -> &'static str {
    match function {
        GerberFileFunction::Assembly(_) => "gerber-file-function-assembly",
        GerberFileFunction::Component(_) => "gerber-file-function-component",
        GerberFileFunction::Copper(_) => "gerber-file-function-copper",
        GerberFileFunction::Legend(_) => "gerber-file-function-legend",
        GerberFileFunction::Paste(_) => "gerber-file-function-paste",
        GerberFileFunction::Profile => "gerber-file-function-profile",
        GerberFileFunction::Solder(_) => "gerber-file-function-solder",
        GerberFileFunction::Other(_) => "gerber-file-function-other",
    }
}

pub fn gerber_file_function_discriminant_to_i18n_key(
    function_discriminant: &GerberFileFunctionDiscriminants,
) -> &'static str {
    match function_discriminant {
        GerberFileFunctionDiscriminants::Assembly => "gerber-file-function-assembly",
        GerberFileFunctionDiscriminants::Component => "gerber-file-function-component",
        GerberFileFunctionDiscriminants::Copper => "gerber-file-function-copper",
        GerberFileFunctionDiscriminants::Legend => "gerber-file-function-legend",
        GerberFileFunctionDiscriminants::Paste => "gerber-file-function-paste",
        GerberFileFunctionDiscriminants::Profile => "gerber-file-function-profile",
        GerberFileFunctionDiscriminants::Solder => "gerber-file-function-solder",
        GerberFileFunctionDiscriminants::Other => "gerber-file-function-other",
    }
}

pub fn pcb_orientation_pitch_flip_to_i18n_key(flip: PcbAssemblyFlip) -> &'static str {
    match flip {
        PcbAssemblyFlip::None => "pcb-assembly-orientation-flip-none",
        PcbAssemblyFlip::Pitch => "pcb-assembly-orientation-flip-pitch",
        PcbAssemblyFlip::Roll => "pcb-assembly-orientation-flip-roll",
    }
}

#[cfg(test)]
mod tests {
    use planner_app::TaskAction;

    use super::process_task_action_to_i18n_key;

    /// `ISSUE-13` showed the actions' names from the code.
    #[test]
    fn every_task_action_is_translated() {
        for language in ["en-US", "es-ES"] {
            let path = format!(
                "{}/assets/translations/{language}/translations.ftl",
                env!("CARGO_MANIFEST_DIR")
            );
            let translations = std::fs::read_to_string(&path).unwrap();

            for action in [TaskAction::Start, TaskAction::Complete, TaskAction::Abandon] {
                let key = process_task_action_to_i18n_key(&action);
                assert!(
                    translations
                        .lines()
                        .any(|line| line.starts_with(&format!("{key} = "))),
                    "'{key}' is not in {path}"
                );
            }
        }
    }
}
