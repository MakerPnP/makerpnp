//! A design variant's panels, docked in its tab as a project's are: its explorer, its "Overview", and a panel for each
//! of its data sources and outputs (`VBGUI-9`).

use std::path::PathBuf;

use egui::WidgetText;
use egui_dock::{DockState, NodeIndex, Split};
use egui_i18n::tr;
use serde::{Deserialize, Serialize};

/// A panel of a design variant's tab, persisted, with the tab, so it's shown again when the application restarts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Panel {
    Explorer,
    /// The variant builder's fields, in groups, which are saved, and built from (`VBGUI-2`).
    Overview,
    /// A data source, as its field names it, and its path, as the variant builder's file has it (`VBGUI-11`).
    DataSource {
        source: DataSource,
        path: PathBuf,
    },
    /// The build's output: the mapping results, to come (`VBGUI-12`).
    BuildOutput,
    /// The variant's placements or BOM, as built (`VBGUI-12`).
    Output(Output),
}

/// A field of a variant builder whose value is a data source, or a list of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DataSource {
    /// The design's placements, from the EDA tool.
    Placements,
    Parts,
    PartMappings,
    GlobalSubstitutions,
    AssemblyRules,
    DesignSubstitutions,
}

/// A variant's outputs, as built (`VARB-9`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Output {
    Placements,
    Bom,
}

impl DataSource {
    /// The field's label, as the "Overview" panel has it.
    pub fn label(&self) -> String {
        match self {
            DataSource::Placements => tr!("variant-builder-input-placements"),
            DataSource::Parts => tr!("variant-builder-input-parts"),
            DataSource::PartMappings => tr!("variant-builder-input-part-mappings"),
            DataSource::GlobalSubstitutions => tr!("variant-builder-input-global-substitutions"),
            DataSource::AssemblyRules => tr!("variant-builder-input-assembly-rules"),
            DataSource::DesignSubstitutions => tr!("variant-builder-input-design-substitutions"),
        }
    }
}

impl Output {
    pub fn label(&self) -> String {
        match self {
            Output::Placements => tr!("variant-builder-panel-output-placements"),
            Output::Bom => tr!("variant-builder-panel-output-bom"),
        }
    }
}

impl Panel {
    /// The panel's title: a data source's is its field's label and its file, "`<field>: <file>`".
    pub fn label(&self) -> WidgetText {
        let label = match self {
            Panel::Explorer => tr!("variant-builder-panel-explorer"),
            Panel::Overview => tr!("variant-builder-panel-overview"),
            Panel::DataSource {
                source,
                path,
            } => format!("{}: {}", source.label(), path.display()),
            Panel::BuildOutput => tr!("variant-builder-panel-build-output"),
            Panel::Output(output) => output.label(),
        };
        WidgetText::from(label)
    }
}

/// The panels, docked: the explorer on the left, and the others on the right, as a project's are.
#[derive(Debug, Serialize, Deserialize)]
pub struct Panels(pub DockState<Panel>);

impl Default for Panels {
    fn default() -> Self {
        Self(DockState::new(vec![]))
    }
}

impl Panels {
    /// The explorer and the "Overview", unless there are panels already, such as those shown when the application
    /// last closed (`VBGUI-9`).
    pub fn ensure_layout(&mut self) {
        if self.0.iter_all_tabs().next().is_some() {
            return;
        }
        self.show(Panel::Explorer);
        self.show(Panel::Overview);
    }

    /// Shows the panel, making it the active one of its node, or opens it, beside the explorer, if it isn't open
    /// (`VBGUI-10`).
    pub fn show(&mut self, panel: Panel) {
        if let Some(path) = self.0.find_tab(&panel) {
            let _ = self.0.set_active_tab(path);
            self.0
                .set_focused_node_and_surface(path.node_path());
            return;
        }

        let surface = self.0.main_surface_mut();
        if surface.is_empty() {
            surface.push_to_first_leaf(panel);
            return;
        }

        // the first leaf that hasn't the explorer, so the others are shown beside it.
        let leaf = self
            .0
            .iter_leaves()
            .find(|(_path, leaf)| !leaf.tabs().contains(&Panel::Explorer))
            .map(|(path, _leaf)| path);
        match leaf {
            Some(path) => {
                if let Ok(leaf) = self.0.leaf_mut(path) {
                    leaf.append_tab(panel.clone());
                }
                if let Some(path) = self.0.find_tab(&panel) {
                    let _ = self.0.set_active_tab(path);
                    self.0
                        .set_focused_node_and_surface(path.node_path());
                }
            }
            None => {
                self.0
                    .main_surface_mut()
                    .split_tabs(NodeIndex::root(), Split::Right, 0.25, vec![panel]);
            }
        }
    }
}
