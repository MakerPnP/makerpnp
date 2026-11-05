//! A design variant's explorer: a tree with a node for the "Overview", each of the variant builder's data sources, in
//! folders named after the "Overview" panel's groups, and its build's outputs, each opening its panel when it's
//! double-clicked, as a project's explorer does (`VBGUI-10`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use egui::Ui;
use egui_i18n::tr;
use egui_ltreeview::{Action, NodeBuilder, TreeView, TreeViewBuilder, TreeViewState};
use variantbuilder_app::VariantBuilderFile;

use super::panels::{DataSource, Output, Panel};

/// Builds the tree, giving each node an id, in order, and keeping the panel each node opens.
struct Nodes<'b, 'a> {
    builder: &'b mut TreeViewBuilder<'a, usize>,
    next_id: usize,
    panels: BTreeMap<usize, Panel>,
}

impl Nodes<'_, '_> {
    fn next_id(&mut self, panel: Option<Panel>) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        if let Some(panel) = panel {
            self.panels.insert(id, panel);
        }
        id
    }

    /// A folder, open, which opens the panel when it's double-clicked.
    fn folder(&mut self, label: String, panel: Option<Panel>) {
        let id = self.next_id(panel);
        self.builder.node(
            NodeBuilder::dir(id)
                .activatable(true)
                .default_open(true)
                .label_ui(move |ui| {
                    ui.add(egui::Label::new(label.as_str()).selectable(false));
                }),
        );
    }

    fn close_folder(&mut self) {
        self.builder.close_dir();
    }

    fn leaf(&mut self, label: String, panel: Option<Panel>) {
        let id = self.next_id(panel);
        self.builder.leaf(id, label);
    }

    /// A data source that's a file, "`<field>: <file>`", or "`<field>: (none)`" if it isn't set, which opens nothing.
    fn file(&mut self, source: DataSource, path: &Option<PathBuf>) {
        match path {
            Some(path) => self.leaf(
                format!("{}: {}", source.label(), path.display()),
                Some(Panel::DataSource {
                    source,
                    path: path.clone(),
                }),
            ),
            None => self.leaf(
                format!("{}: {}", source.label(), tr!("variant-builder-explorer-none")),
                None,
            ),
        }
    }

    /// A list of data sources, a folder named after the field, opening the "Overview", where the list is changed, with
    /// a node for each file, in order.
    fn files(&mut self, source: DataSource, paths: &[PathBuf]) {
        self.folder(source.label(), Some(Panel::Overview));
        for path in paths {
            self.leaf(
                path.display().to_string(),
                Some(Panel::DataSource {
                    source,
                    path: path.clone(),
                }),
            );
        }
        self.close_folder();
    }
}

/// Shows the tree of the variant builder, as its file says, `name` being the file's name, and returns the panel of the
/// node double-clicked, if one was.
pub fn show_explorer(
    ui: &mut Ui,
    name: &str,
    file: &VariantBuilderFile,
    state: &mut TreeViewState<usize>,
) -> Option<Panel> {
    let mut panels = BTreeMap::new();

    let (_response, actions) = TreeView::new(ui.make_persistent_id("variant_builder_explorer_tree"))
        .allow_multi_selection(false)
        .show_state(ui, state, |builder| {
            let mut nodes = Nodes {
                builder,
                next_id: 0,
                panels: BTreeMap::new(),
            };

            // the root, the variant builder, shows its "Overview".
            nodes.folder(name.to_string(), Some(Panel::Overview));
            nodes.leaf(tr!("variant-builder-panel-overview"), Some(Panel::Overview));

            // what it's of, the "Overview"'s first group, which has no title.
            nodes.file(DataSource::Placements, &file.placements);

            nodes.folder(tr!("variant-builder-group-part-matching"), Some(Panel::Overview));
            nodes.file(DataSource::Parts, &file.parts);
            nodes.file(DataSource::PartMappings, &file.part_mappings);
            nodes.files(DataSource::GlobalSubstitutions, &file.global_substitutions);
            nodes.close_folder();

            nodes.folder(tr!("variant-builder-group-design-specific"), Some(Panel::Overview));
            nodes.file(DataSource::AssemblyRules, &file.assembly_rules);
            nodes.files(DataSource::DesignSubstitutions, &file.design_substitutions);
            nodes.close_folder();

            nodes.folder(tr!("variant-builder-panel-build-output"), Some(Panel::BuildOutput));
            nodes.leaf(Output::Placements.label(), Some(Panel::Output(Output::Placements)));
            nodes.leaf(Output::Bom.label(), Some(Panel::Output(Output::Bom)));
            nodes.close_folder();

            // root
            nodes.close_folder();

            panels = nodes.panels;
        });

    let mut activated = None;
    for action in actions {
        if let Action::Activate(activation) = action {
            for node_id in activation.selected {
                if let Some(panel) = panels.get(&node_id) {
                    activated = Some(panel.clone());
                }
                // HACK: tree-view-dir-activate-expand-hack
                state.expand_node(&node_id);
            }
        }
    }
    activated
}
