use egui::{Checkbox, FontFamily, RichText, Ui, WidgetText};
use egui_i18n::tr;
use egui_material_icons::icons::ICON_HOME;
use egui_taffy::taffy::Style;
use egui_taffy::taffy::prelude::{length, percent};
use egui_taffy::{TuiBuilderLogic, taffy, tui};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::config::Config;
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct HomeTab {
    show_on_startup: bool,

    #[serde(skip)]
    pub component: ComponentState<HomeTabUiCommand>,
}

#[derive(Debug, Clone)]
pub enum HomeTabUiCommand {
    None,
    SetShowOnStartup(bool),
}

#[derive(Debug)]
pub enum HomeTabAction {
    None,
}

pub struct HomeTabContext {
    pub tab_key: TabKey,
    pub config: Value<Config>,
}

impl Tab for HomeTab {
    type Context = HomeTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("tab-label-home"))
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        let mut home_tab_context = HomeTabContext {
            tab_key: tab_key.clone(),
            config: context.config.clone(),
        };
        UiComponent::ui(self, ui, &mut home_tab_context);
    }
}

impl UiComponent for HomeTab {
    type UiContext<'context> = HomeTabContext;
    type UiCommand = HomeTabUiCommand;
    type UiAction = HomeTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        ui.ctx().global_style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let default_style = || Style {
            padding: length(8.),
            gap: length(8.),
            ..Default::default()
        };

        tui(ui, ui.id().with("home"))
            .reserve_available_space()
            .style(Style {
                justify_content: Some(taffy::JustifyContent::Center),
                align_items: Some(taffy::AlignItems::Center),
                flex_direction: taffy::FlexDirection::Column,
                size: taffy::Size {
                    width: percent(1.),
                    height: percent(1.),
                },
                ..default_style()
            })
            .show(|tui| {
                tui.style(Style {
                    flex_direction: taffy::FlexDirection::Row,
                    //align_self: Some(taffy::AlignItems::Center),
                    ..default_style()
                })
                .add_with_border(|tui| {
                    tui.label(
                        RichText::new(ICON_HOME)
                            .size(48.0)
                            .family(FontFamily::Proportional),
                    );
                    tui.label(
                        RichText::new(tr!("home-banner"))
                            .size(48.0)
                            .family(FontFamily::Proportional),
                    );
                });

                tui.ui(|ui| {
                    let mut show_home_tab_on_startup = context
                        .config
                        .lock()
                        .unwrap()
                        .show_home_tab_on_startup;
                    if ui
                        .add(Checkbox::new(
                            &mut show_home_tab_on_startup,
                            tr!("home-checkbox-label-show-on-startup"),
                        ))
                        .changed()
                    {
                        self.component
                            .send(HomeTabUiCommand::SetShowOnStartup(show_home_tab_on_startup));
                    }
                });
            });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("HomeTab command. command: {:?}", command);
        match command {
            HomeTabUiCommand::None => Some(HomeTabAction::None),
            HomeTabUiCommand::SetShowOnStartup(value) => {
                context
                    .config
                    .lock()
                    .unwrap()
                    .show_home_tab_on_startup = value;
                None
            }
        }
    }
}
