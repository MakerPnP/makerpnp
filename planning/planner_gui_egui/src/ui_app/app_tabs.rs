use std::path::PathBuf;
use std::sync::Arc;

use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, NodeIndex, Split, Style};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use tracing::{debug, error};

use crate::config::Config;
use crate::file_picker::FileDialogs;
use crate::pcb::{Pcb, PcbKey};
use crate::process::{Process, ProcessKey};
use crate::project::{Project, ProjectKey};
use crate::stores_core::StoresCoreService;
use crate::sync::{Enqueue, Value, ValueGuard};
use crate::tabs::{AppTabViewer, Tab, TabKey, Tabs};
use crate::ui_app::app_tabs::home::{HomeTab, HomeTabAction, HomeTabContext, HomeTabUiCommand};
use crate::ui_app::app_tabs::new_pcb::{NewPcbTab, NewPcbTabAction, NewPcbTabContext, NewPcbTabUiCommand};
use crate::ui_app::app_tabs::new_process::{
    NewProcessTab, NewProcessTabAction, NewProcessTabContext, NewProcessTabUiCommand,
};
use crate::ui_app::app_tabs::new_project::{
    NewProjectTab, NewProjectTabAction, NewProjectTabContext, NewProjectTabUiCommand,
};
use crate::ui_app::app_tabs::parts::{PartsTab, PartsTabAction, PartsTabContext, PartsTabUiCommand};
use crate::ui_app::app_tabs::pcb::{PcbTab, PcbTabAction, PcbTabUiCommand};
use crate::ui_app::app_tabs::process::{ProcessTab, ProcessTabAction, ProcessTabUiCommand};
use crate::ui_app::app_tabs::project::{ProjectTab, ProjectTabAction, ProjectTabUiCommand};
use crate::ui_component::{ComponentState, UiComponent};

pub mod home;
pub mod new_pcb;
pub mod new_process;
pub mod new_project;
pub mod parts;
pub mod pcb;
pub mod process;
pub mod project;

#[derive(Debug)]
pub struct TabKindContext {
    pub config: Value<Config>,
    pub projects: Value<SlotMap<ProjectKey, Project>>,
    pub pcbs: Value<SlotMap<PcbKey, Pcb>>,
    pub processes: Value<SlotMap<ProcessKey, Process>>,
    /// The application's one stores core, shared by every tab.
    pub stores: Value<StoresCoreService>,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum TabKind {
    Home(HomeTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    NewProject(NewProjectTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    Project(ProjectTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    NewPcb(NewPcbTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    Pcb(PcbTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    NewProcess(NewProcessTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    Process(ProcessTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    Parts(PartsTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
}

#[derive(Debug, Clone)]
pub enum TabKindUiCommand {
    HomeTabCommand { command: HomeTabUiCommand },
    NewProjectTabCommand { command: NewProjectTabUiCommand },
    ProjectTabCommand { command: ProjectTabUiCommand },
    NewPcbTabCommand { command: NewPcbTabUiCommand },
    PcbTabCommand { command: PcbTabUiCommand },
    NewProcessTabCommand { command: NewProcessTabUiCommand },
    ProcessTabCommand { command: ProcessTabUiCommand },
    PartsTabCommand { command: PartsTabUiCommand },
}

#[derive(Debug)]
pub enum TabKindAction {
    None,
    HomeTabAction { action: HomeTabAction },
    NewProjectTabAction { action: NewProjectTabAction },
    ProjectTabAction { action: ProjectTabAction },
    NewPcbTabAction { action: NewPcbTabAction },
    PcbTabAction { action: PcbTabAction },
    NewProcessTabAction { action: NewProcessTabAction },
    ProcessTabAction { action: ProcessTabAction },
    PartsTabAction { action: PartsTabAction },
}

impl Tab for TabKind {
    type Context = TabKindContext;

    fn label(&self) -> WidgetText {
        match self {
            TabKind::Home(tab, _) => tab.label(),
            TabKind::NewProject(tab, _) => tab.label(),
            TabKind::Project(tab, _) => tab.label(),
            TabKind::NewPcb(tab, _) => tab.label(),
            TabKind::Pcb(tab, _) => tab.label(),
            TabKind::NewProcess(tab, _) => tab.label(),
            TabKind::Process(tab, _) => tab.label(),
            TabKind::Parts(tab, _) => tab.label(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        UiComponent::ui(self, ui, &mut (tab_key.clone(), context));
    }

    fn on_close(&mut self, tab_key: &TabKey, context: &mut Self::Context) -> OnCloseResponse {
        match self {
            TabKind::Home(tab, _) => {
                let mut home_tab_context = HomeTabContext {
                    tab_key: tab_key.clone(),
                    config: context.config.clone(),
                };
                tab.on_close(tab_key, &mut home_tab_context)
            }
            TabKind::NewProject(tab, _) => {
                let mut new_project_tab_context = NewProjectTabContext {
                    tab_key: tab_key.clone(),
                };
                tab.on_close(tab_key, &mut new_project_tab_context)
            }
            TabKind::Project(tab, _) => {
                let mut project_tab_context = project::ProjectTabContext {
                    tab_key: tab_key.clone(),
                    projects: context.projects.clone(),
                };
                tab.on_close(tab_key, &mut project_tab_context)
            }
            TabKind::NewPcb(tab, _) => {
                let mut new_pcb_tab_context = NewPcbTabContext {
                    tab_key: tab_key.clone(),
                };
                tab.on_close(tab_key, &mut new_pcb_tab_context)
            }
            TabKind::Pcb(tab, _) => {
                let mut pcb_tab_context = pcb::PcbTabContext {
                    tab_key: tab_key.clone(),
                    pcbs: context.pcbs.clone(),
                };
                tab.on_close(tab_key, &mut pcb_tab_context)
            }
            TabKind::NewProcess(tab, _) => {
                let mut new_process_tab_context = NewProcessTabContext {
                    tab_key: tab_key.clone(),
                };
                tab.on_close(tab_key, &mut new_process_tab_context)
            }
            TabKind::Process(tab, _) => {
                let mut process_tab_context = process::ProcessTabContext {
                    tab_key: tab_key.clone(),
                    processes: context.processes.clone(),
                };
                tab.on_close(tab_key, &mut process_tab_context)
            }
            TabKind::Parts(tab, _) => {
                let mut parts_tab_context = PartsTabContext {
                    tab_key: tab_key.clone(),
                    stores: context.stores.clone(),
                };
                tab.on_close(tab_key, &mut parts_tab_context)
            }
        }
    }
}

impl UiComponent for TabKind {
    type UiContext<'context> = (TabKey, &'context mut TabKindContext);
    type UiCommand = (TabKey, TabKindUiCommand);
    type UiAction = TabKindAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let (tab_key, context) = context;
        let tab_key = *tab_key;

        match self {
            TabKind::Home(tab, _) => {
                let mut home_tab_context = HomeTabContext {
                    tab_key,
                    config: context.config.clone(),
                };
                tab.ui(ui, &mut home_tab_context)
            }
            TabKind::NewProject(tab, _) => {
                let mut new_project_tab_context = NewProjectTabContext {
                    tab_key,
                };
                tab.ui(ui, &mut new_project_tab_context)
            }
            TabKind::Project(tab, _) => {
                let mut project_tab_context = project::ProjectTabContext {
                    tab_key,
                    projects: context.projects.clone(),
                };
                tab.ui(ui, &mut project_tab_context)
            }
            TabKind::NewPcb(tab, _) => {
                let mut new_pcb_tab_context = NewPcbTabContext {
                    tab_key,
                };
                tab.ui(ui, &mut new_pcb_tab_context)
            }
            TabKind::Pcb(tab, _) => {
                let mut pcb_tab_context = pcb::PcbTabContext {
                    tab_key,
                    pcbs: context.pcbs.clone(),
                };
                tab.ui(ui, &mut pcb_tab_context)
            }
            TabKind::NewProcess(tab, _) => {
                let mut new_process_tab_context = NewProcessTabContext {
                    tab_key,
                };
                tab.ui(ui, &mut new_process_tab_context)
            }
            TabKind::Process(tab, _) => {
                let mut process_tab_context = process::ProcessTabContext {
                    tab_key,
                    processes: context.processes.clone(),
                };
                tab.ui(ui, &mut process_tab_context)
            }
            TabKind::Parts(tab, _) => {
                let mut parts_tab_context = PartsTabContext {
                    tab_key,
                    stores: context.stores.clone(),
                };
                tab.ui(ui, &mut parts_tab_context)
            }
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        let (tab_key, command) = command;
        let (_tab_key, context) = context;

        match (self, command) {
            (
                TabKind::Home(tab, _),
                TabKindUiCommand::HomeTabCommand {
                    command,
                },
            ) => {
                let mut home_tab_context = HomeTabContext {
                    tab_key,
                    config: context.config.clone(),
                };
                tab.update(command, &mut home_tab_context)
                    .map(|action| TabKindAction::HomeTabAction {
                        action,
                    })
            }
            (
                TabKind::NewProject(tab, _),
                TabKindUiCommand::NewProjectTabCommand {
                    command,
                },
            ) => {
                let mut new_project_tab_content = NewProjectTabContext {
                    tab_key,
                };
                tab.update(command, &mut new_project_tab_content)
                    .map(|action| TabKindAction::NewProjectTabAction {
                        action,
                    })
            }
            (
                TabKind::Project(tab, _),
                TabKindUiCommand::ProjectTabCommand {
                    command,
                },
            ) => {
                let mut project_tab_context = project::ProjectTabContext {
                    tab_key,
                    projects: context.projects.clone(),
                };
                tab.update(command, &mut project_tab_context)
                    .map(|action| TabKindAction::ProjectTabAction {
                        action,
                    })
            }
            (
                TabKind::NewPcb(tab, _),
                TabKindUiCommand::NewPcbTabCommand {
                    command,
                },
            ) => {
                let mut new_pcb_tab_content = NewPcbTabContext {
                    tab_key,
                };
                tab.update(command, &mut new_pcb_tab_content)
                    .map(|action| TabKindAction::NewPcbTabAction {
                        action,
                    })
            }
            (
                TabKind::Pcb(tab, _),
                TabKindUiCommand::PcbTabCommand {
                    command,
                },
            ) => {
                let mut pcb_tab_context = pcb::PcbTabContext {
                    tab_key,
                    pcbs: context.pcbs.clone(),
                };
                tab.update(command, &mut pcb_tab_context)
                    .map(|action| TabKindAction::PcbTabAction {
                        action,
                    })
            }
            (
                TabKind::NewProcess(tab, _),
                TabKindUiCommand::NewProcessTabCommand {
                    command,
                },
            ) => {
                let mut new_process_tab_context = NewProcessTabContext {
                    tab_key,
                };
                tab.update(command, &mut new_process_tab_context)
                    .map(|action| TabKindAction::NewProcessTabAction {
                        action,
                    })
            }
            (
                TabKind::Process(tab, _),
                TabKindUiCommand::ProcessTabCommand {
                    command,
                },
            ) => {
                let mut process_tab_context = process::ProcessTabContext {
                    tab_key,
                    processes: context.processes.clone(),
                };
                tab.update(command, &mut process_tab_context)
                    .map(|action| TabKindAction::ProcessTabAction {
                        action,
                    })
            }
            (
                TabKind::Parts(tab, _),
                TabKindUiCommand::PartsTabCommand {
                    command,
                },
            ) => {
                let mut parts_tab_context = PartsTabContext {
                    tab_key,
                    stores: context.stores.clone(),
                };
                tab.update(command, &mut parts_tab_context)
                    .map(|action| TabKindAction::PartsTabAction {
                        action,
                    })
            }
            _ => {
                // this can occur when adding new tab kinds or when the types are mismatched
                unreachable!()
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct AppTabs {
    tabs: Value<Tabs<TabKind, TabKindContext>>,

    // Note: `tree` is wrapped in a value because `ui()` only gives us `&self`
    //       but dockstate needs a mutable tree.
    tree: Value<DockState<TabKey>>,

    #[serde(skip)]
    pub component: ComponentState<(TabKey, TabUiCommand)>,
}

impl Default for AppTabs {
    fn default() -> Self {
        // TODO dockstate needs translations, see https://docs.rs/egui_dock/latest/egui_dock/#translations

        Self {
            tabs: Value::new(Tabs::new()),
            tree: Value::new(DockState::new(vec![])),
            component: ComponentState::default(),
        }
    }
}

#[macro_export]
macro_rules! tabs_impl {
    ( $tab_kind:ident, $tab_context:ident ) => {
        //
        // methods common to all tab kinds
        //
        pub fn restore(&mut self, other: &mut Self) {
            tracing::debug!("Restoring tabs. self: {:?}, other: {:?}", self, other);

            std::mem::swap(&mut self.tabs, &mut other.tabs);
            std::mem::swap(&mut self.tree, &mut other.tree);
        }

        #[allow(dead_code)]
        pub fn close_all_tabs(&mut self) {
            let mut tree = self.tree.lock().unwrap();

            // FIXME there's a bug in `egui_dock` where the `on_close` handler is not called
            //       when programmatically closing all the tabs - reported via discord: https://discord.com/channels/900275882684477440/1075333382290026567/1340993744941617233
            tree.retain_tabs(|_tab_key| false)
        }

        /// Due to bugs in egui_dock where it doesn't call `on_close` when closing tabs, it's possible that the tabs
        /// and the dock tree are out of sync.  `on_close` should be removing elements from `self.tabs` corresponding to the
        /// tab being closed, but because it is not called there can be orphaned elements, we need to find and remove them.
        pub fn cleanup_tabs(&mut self, tab_context: &mut $tab_context) {
            let tree = self.tree.lock().unwrap();

            let known_tab_keys = tree
                .iter_all_tabs()
                .map(|(_surface_and_node, tab_key)| tab_key.clone())
                .collect::<Vec<_>>();

            let mut tabs = self.tabs.lock().unwrap();

            tabs.retain_all(&known_tab_keys, tab_context)
        }

        pub fn add_tab(&mut self, tab_kind: $tab_kind) -> TabKey {
            let mut tabs = self.tabs.lock().unwrap();
            let tab_key = tabs.add(tab_kind);

            let mut tree = self.tree.lock().unwrap();
            tree.push_to_focused_leaf(tab_key);

            tab_key
        }

        #[allow(dead_code)]
        pub fn find_tab<F>(&self, f: F) -> Option<TabKey>
        where
            F: Fn(&$tab_kind) -> bool,
        {
            let tree = self.tree.lock().unwrap();

            let tab = tree
                .iter_all_tabs()
                .find_map(|(_surface_and_node, tab_key)| {
                    let tabs = self.tabs.lock().unwrap();
                    let tab_kind = tabs.get(tab_key).unwrap();

                    match f(tab_kind) {
                        true => Some(*tab_key),
                        false => None,
                    }
                });

            tab
        }

        #[allow(dead_code)]
        pub fn filter_map<B, F>(&self, f: F) -> Vec<B>
        where
            F: FnMut((&TabKey, &$tab_kind)) -> Option<B>,
        {
            let tabs = self.tabs.lock().unwrap();
            tabs.iter()
                .filter_map(f)
                .collect::<Vec<_>>()
        }

        #[allow(dead_code)]
        pub fn filter_map_mut<B, F>(&self, f: F) -> Vec<B>
        where
            F: FnMut((&TabKey, &mut $tab_kind)) -> Option<B>,
        {
            let mut tabs = self.tabs.lock().unwrap();
            tabs.iter_mut()
                .filter_map(f)
                .collect::<Vec<_>>()
        }

        #[allow(dead_code)]
        pub fn with_tab_mut<F, O>(&self, tab_key: &TabKey, f: F) -> O
        where
            F: Fn(&mut $tab_kind) -> O,
        {
            let mut tabs = self.tabs.lock().unwrap();
            let mut tab = tabs.get_mut(tab_key).unwrap();
            f(&mut tab)
        }

        #[allow(dead_code)]
        pub fn show_tab<F>(&mut self, f: F) -> Result<TabKey, ()>
        where
            F: Fn(&$tab_kind) -> bool,
        {
            let tab = self.find_tab(f);

            let mut tree = self.tree.lock().unwrap();

            if let Some(tab_key) = tab {
                let find_result = tree.find_tab(&tab_key).unwrap();
                tree.set_active_tab(find_result)
                    .unwrap();
                Ok(tab_key)
            } else {
                Err(())
            }
        }

        #[allow(dead_code)]
        pub fn active_tab(&self) -> Option<TabKey> {
            let mut tree = self.tree.lock().unwrap();
            tree.find_active_focused()
                .map(|(_, tab_key)| tab_key.clone())
        }

        #[allow(dead_code)]
        pub fn retain<F>(&mut self, f: F)
        where
            F: Fn(&TabKey, &$tab_kind) -> bool,
        {
            let tabs = self.tabs.lock().unwrap();
            let tab_keys_to_retain = tabs
                .iter()
                .filter_map(|(tab_key, tab_kind)| match f(tab_key, tab_kind) {
                    true => Some(tab_key.clone()),
                    false => None,
                })
                .collect::<Vec<_>>();

            let mut tree = self.tree.lock().unwrap();
            tree.retain_tabs(|tab_key| tab_keys_to_retain.contains(&tab_key));
        }

        // FIXME This only works when all the tabs are closed and they are added in a particular order.
        //       this is because the user can move tabs between surfaces and nodes, so later trying to use
        //       a desired surface will not do what you want.
        #[allow(dead_code)]
        pub fn add_tab_to_leaf_or_split(&mut self, tab_kind: $tab_kind, parent_fraction: f32, split: Split) -> TabKey {
            let mut tabs = self.tabs.lock().unwrap();
            let tab_key = tabs.add(tab_kind);

            let mut tree = self.tree.lock().unwrap();

            let surface = tree.main_surface_mut();
            if surface.is_empty() {
                surface.push_to_first_leaf(tab_key);
                return tab_key;
            }

            let mut leaf_index = 0;
            for (_node_index, node) in surface.iter_mut().enumerate() {
                if !node.is_leaf() {
                    continue;
                }

                if leaf_index > 0 {
                    node.append_tab(tab_key);
                    return tab_key;
                }

                leaf_index += 1;
            }

            let [_old_node_index, _new_node_index] =
                surface.split_tabs(NodeIndex::root(), split, parent_fraction, vec![tab_key]);
            tab_key
        }
    };
}

impl AppTabs {
    tabs_impl!(TabKind, TabKindContext);

    pub fn replace(&mut self, tab_key: &TabKey, replacement_tab_kind: TabKind) -> Result<(), ()> {
        let mut tabs = self.tabs.lock().unwrap();

        if let Some(tab_kind) = tabs.get_mut(tab_key) {
            *tab_kind = replacement_tab_kind;
            Ok(())
        } else {
            Err(())
        }
    }
    //
    // methods specific to this instance
    //

    pub fn add_new_project_tab(&mut self, file_dialogs: Arc<dyn FileDialogs>) {
        // create a new project tab
        let tab_kind_component = ComponentState::default();

        let mut tabs = self.tabs.lock().unwrap();
        let new_project_tab = NewProjectTab::new(file_dialogs);
        let tab_kind = TabKind::NewProject(new_project_tab, tab_kind_component);
        let tab_key = tabs.add(tab_kind);

        let tab_kind_sender = self.component.sender.clone();
        Self::configure_new_project_tab_mappers(tab_kind_sender, tabs, tab_key);

        let mut tree = self.tree.lock().unwrap();
        tree.push_to_focused_leaf(tab_key);
    }

    pub fn add_new_pcb_tab(&mut self, file_dialogs: Arc<dyn FileDialogs>) {
        // create a new pcb tab
        let tab_kind_component = ComponentState::default();

        let mut tabs = self.tabs.lock().unwrap();
        let new_pcb_tab = NewPcbTab::new(file_dialogs);
        let tab_kind = TabKind::NewPcb(new_pcb_tab, tab_kind_component);
        let tab_key = tabs.add(tab_kind);

        let tab_kind_sender = self.component.sender.clone();
        Self::configure_new_pcb_tab_mappers(tab_kind_sender, tabs, tab_key);

        let mut tree = self.tree.lock().unwrap();
        tree.push_to_focused_leaf(tab_key);
    }

    pub fn add_new_process_tab(&mut self, file_dialogs: Arc<dyn FileDialogs>) {
        let tab_kind_component = ComponentState::default();

        let mut tabs = self.tabs.lock().unwrap();
        let new_process_tab = NewProcessTab::new(file_dialogs);
        let tab_kind = TabKind::NewProcess(new_process_tab, tab_kind_component);
        let tab_key = tabs.add(tab_kind);

        let tab_kind_sender = self.component.sender.clone();
        Self::configure_new_process_tab_mappers(tab_kind_sender, tabs, tab_key);

        let mut tree = self.tree.lock().unwrap();
        tree.push_to_focused_leaf(tab_key);
    }

    pub fn add_parts_tab(&mut self, file_dialogs: Arc<dyn FileDialogs>) {
        let tab_kind_component = ComponentState::default();

        let mut tabs = self.tabs.lock().unwrap();
        let parts_tab = PartsTab::new(file_dialogs);
        let tab_kind = TabKind::Parts(parts_tab, tab_kind_component);
        let tab_key = tabs.add(tab_kind);

        let tab_kind_sender = self.component.sender.clone();
        Self::configure_parts_tab_mappers(tab_kind_sender, &mut tabs, tab_key);

        let mut tree = self.tree.lock().unwrap();
        tree.push_to_focused_leaf(tab_key);
    }

    /// The parts tabs restored when the application starts aren't wired up, nor have file dialogs, as they were
    /// restored with `Default`.  Returns the tabs whose data source had been chosen, so their parts can be loaded.
    ///
    /// Safety: call only once on startup, before the tabs are shown.
    pub fn restore_parts_tabs_on_startup(&mut self, file_dialogs: Arc<dyn FileDialogs>) -> Vec<TabKey> {
        let mut tabs = self.tabs.lock().unwrap();

        let tab_keys = tabs
            .iter()
            .filter_map(|(tab_key, tab_kind)| matches!(tab_kind, TabKind::Parts(..)).then_some(*tab_key))
            .collect::<Vec<_>>();

        let tab_kind_sender = self.component.sender.clone();
        let mut tab_keys_with_sources = vec![];
        for tab_key in tab_keys {
            Self::configure_parts_tab_mappers(tab_kind_sender.clone(), &mut tabs, tab_key);

            if let Some(TabKind::Parts(parts_tab, _)) = tabs.get_mut(&tab_key) {
                parts_tab.set_file_dialogs(file_dialogs.clone());
                if parts_tab.source.is_some() {
                    tab_keys_with_sources.push(tab_key);
                }
            }
        }

        tab_keys_with_sources
    }

    fn configure_parts_tab_mappers(
        tab_kind_sender: Enqueue<(TabKey, TabUiCommand)>,
        tabs: &mut ValueGuard<Tabs<TabKind, TabKindContext>>,
        tab_key: TabKey,
    ) {
        match tabs.tabs.get_mut(&tab_key).unwrap() {
            TabKind::Parts(parts_tab, tab_kind_component) => {
                tab_kind_component.configure_mapper(tab_kind_sender, move |command| {
                    (tab_key, TabUiCommand::TabKindCommand(command))
                });

                let tab_kind_component_sender = tab_kind_component.sender.clone();

                parts_tab
                    .component
                    .configure_mapper(tab_kind_component_sender, move |command| {
                        TabKindUiCommand::PartsTabCommand {
                            command,
                        }
                    });
            }
            _ => unreachable!(),
        }
    }

    fn configure_new_process_tab_mappers(
        tab_kind_sender: Enqueue<(TabKey, TabUiCommand)>,
        mut tabs: ValueGuard<Tabs<TabKind, TabKindContext>>,
        tab_key: TabKey,
    ) {
        match tabs.tabs.get_mut(&tab_key).unwrap() {
            TabKind::NewProcess(new_process_tab, tab_kind_component) => {
                tab_kind_component.configure_mapper(tab_kind_sender, move |command| {
                    (tab_key, TabUiCommand::TabKindCommand(command))
                });

                let tab_kind_component_sender = tab_kind_component.sender.clone();

                new_process_tab
                    .component
                    .configure_mapper(tab_kind_component_sender, move |command| {
                        TabKindUiCommand::NewProcessTabCommand {
                            command,
                        }
                    });
            }
            _ => unreachable!(),
        }
    }

    fn configure_new_project_tab_mappers(
        tab_kind_sender: Enqueue<(TabKey, TabUiCommand)>,
        mut tabs: ValueGuard<Tabs<TabKind, TabKindContext>>,
        tab_key: TabKey,
    ) {
        match tabs.tabs.get_mut(&tab_key).unwrap() {
            TabKind::NewProject(new_project_tab, tab_kind_component) => {
                tab_kind_component.configure_mapper(tab_kind_sender, move |command| {
                    (tab_key, TabUiCommand::TabKindCommand(command))
                });

                let tab_kind_component_sender = tab_kind_component.sender.clone();

                new_project_tab
                    .component
                    .configure_mapper(tab_kind_component_sender, move |command| {
                        TabKindUiCommand::NewProjectTabCommand {
                            command,
                        }
                    });
            }
            _ => unreachable!(),
        }
    }

    fn configure_new_pcb_tab_mappers(
        tab_kind_sender: Enqueue<(TabKey, TabUiCommand)>,
        mut tabs: ValueGuard<Tabs<TabKind, TabKindContext>>,
        tab_key: TabKey,
    ) {
        match tabs.tabs.get_mut(&tab_key).unwrap() {
            TabKind::NewPcb(new_pcb_tab, tab_kind_component) => {
                tab_kind_component.configure_mapper(tab_kind_sender, move |command| {
                    (tab_key, TabUiCommand::TabKindCommand(command))
                });

                let tab_kind_component_sender = tab_kind_component.sender.clone();

                new_pcb_tab
                    .component
                    .configure_mapper(tab_kind_component_sender, move |command| {
                        TabKindUiCommand::NewPcbTabCommand {
                            command,
                        }
                    });
            }
            _ => unreachable!(),
        }
    }

    pub fn show_home_tab(&mut self) {
        self.show_tab(|candidate_tab| matches!(candidate_tab, TabKind::Home(..)))
            .inspect_err(|_| {
                // create a new home tab
                let tab_kind_component = ComponentState::default();

                let mut tabs = self.tabs.lock().unwrap();
                let home_tab = HomeTab::default();
                let tab_kind = TabKind::Home(home_tab, tab_kind_component);
                let tab_key = tabs.add(tab_kind);

                let tab_kind_sender = self.component.sender.clone();
                Self::configure_home_tab_mappers(tab_kind_sender, tabs, tab_key);

                let mut tree = self.tree.lock().unwrap();
                tree.push_to_focused_leaf(tab_key);
            })
            .ok();
    }

    pub fn show_pcb_tab(&mut self, path: &PathBuf) -> Result<TabKey, ()> {
        self.show_tab(|candidate_tab| matches!(candidate_tab, TabKind::Pcb(tab, _) if tab.path.eq(path)))
    }

    pub fn show_process_tab(&mut self, path: &PathBuf) -> Result<TabKey, ()> {
        self.show_tab(|candidate_tab| matches!(candidate_tab, TabKind::Process(tab, _) if tab.path.eq(path)))
    }

    fn configure_home_tab_mappers(
        tab_kind_sender: Enqueue<(TabKey, TabUiCommand)>,
        mut tabs: ValueGuard<Tabs<TabKind, TabKindContext>>,
        tab_key: TabKey,
    ) {
        match tabs.tabs.get_mut(&tab_key).unwrap() {
            TabKind::Home(home_tab, tab_kind_component) => {
                tab_kind_component.configure_mapper(tab_kind_sender, move |command| {
                    (tab_key, TabUiCommand::TabKindCommand(command))
                });

                let tab_kind_component_sender = tab_kind_component.sender.clone();

                home_tab
                    .component
                    .configure_mapper(tab_kind_component_sender, move |command| {
                        TabKindUiCommand::HomeTabCommand {
                            command,
                        }
                    });
            }
            _ => unreachable!(),
        }
    }

    pub fn find_home_tab(&self) -> Option<TabKey> {
        self.find_tab(|candidate_tab| matches!(candidate_tab, TabKind::Home(..)))
    }

    /// Safety: call only once on startup, before the tabs are shown.
    pub fn show_home_tab_on_startup(&mut self, show_home_tab_on_startup: bool) {
        if show_home_tab_on_startup {
            // the home tab's components won't be wired up, because they got restored with `Default`, so we have to fix it
            if let Some(home_tab_key) = self.find_home_tab() {
                let tabs = self.tabs.lock().unwrap();
                let tab_kind_sender = self.component.sender.clone();
                Self::configure_home_tab_mappers(tab_kind_sender, tabs, home_tab_key);
            }

            self.show_home_tab();
        } else if let Some(home_tab_key) = self.find_home_tab() {
            // TODO refactor into 'retain_tabs(...)` or `remove_tab(...)`
            let mut tree = self.tree.lock().unwrap();
            let find_result = tree.find_tab(&home_tab_key).unwrap();

            tree.remove_tab(find_result);
        }
    }
}

#[derive(Debug, Clone)]
pub enum TabUiCommand {
    TabKindCommand(TabKindUiCommand),
}

#[derive(Debug)]
pub enum TabAction {
    None,
    TabKindAction { action: TabKindAction },
}

impl UiComponent for AppTabs {
    type UiContext<'context> = TabKindContext;
    type UiCommand = (TabKey, TabUiCommand);
    type UiAction = TabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let ctx = ui.ctx();

        let mut app_tab_viewer = AppTabViewer {
            tabs: self.tabs.clone(),
            context,
        };

        let mut tree = self.tree.lock().unwrap();

        DockArea::new(&mut tree)
            .id(ui.id().with("app-tabs"))
            .style(Style::from_egui(ctx.global_style().as_ref()))
            .show_inside(ui, &mut app_tab_viewer);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("AppTabs command. command: {:?}", command);
        let (tab_key, command) = command;
        match command {
            TabUiCommand::TabKindCommand(tab_kind_command) => {
                let mut tabs = self.tabs.lock().unwrap();
                if let Some(tab_kind) = tabs.get_mut(&tab_key) {
                    let tab_action = tab_kind.update((tab_key, tab_kind_command), &mut (tab_key, context));

                    tab_action.map(|action| TabAction::TabKindAction {
                        action,
                    })
                } else {
                    error!("tab not found: {:?}", tab_key);
                    None
                }
            }
        }
    }
}
