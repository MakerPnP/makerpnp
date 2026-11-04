use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use derivative::Derivative;
use egui::{Frame, Margin, Style, Ui};
use egui_i18n::tr;
use regex::{Regex, RegexBuilder};
use tracing::debug;

use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Hash)]
pub struct Filter {
    expression: String,
    mode: FilterMode,

    #[derivative(Hash = "ignore")]
    pub component_state: ComponentState<FilterUiCommand>,
}

impl Filter {
    pub fn does_mode_support_case_sensitivity(&self) -> bool {
        match self.mode {
            FilterMode::PartialMatch {
                ..
            } => true,
            FilterMode::RegexMatch {
                ..
            } => true,
        }
    }

    pub fn is_case_sensitive(&self) -> bool {
        match &self.mode {
            FilterMode::PartialMatch {
                flags,
            } => flags
                .get(&PartialMatchFlag::CaseSensitive)
                .is_some(),
            FilterMode::RegexMatch {
                flags, ..
            } => flags
                .get(&RegexFilterFlag::CaseSensitive)
                .is_some(),
        }
    }

    pub fn set_case_sensitivity(&mut self, enabled: bool) {
        match &mut self.mode {
            FilterMode::PartialMatch {
                flags,
            } => {
                match enabled {
                    true => flags.insert(PartialMatchFlag::CaseSensitive),
                    false => flags.remove(&PartialMatchFlag::CaseSensitive),
                };
            }
            FilterMode::RegexMatch {
                flags, ..
            } => {
                match enabled {
                    true => flags.insert(RegexFilterFlag::CaseSensitive),
                    false => flags.remove(&RegexFilterFlag::CaseSensitive),
                };
            }
        }
    }

    pub fn on_expression_changed(&mut self) {
        let is_case_sensitive = !self.is_case_sensitive();

        match &mut self.mode {
            FilterMode::PartialMatch {
                ..
            } => {}
            FilterMode::RegexMatch {
                flags: _flags,
                regex,
            } => {
                if let Ok(parsed_regex) = RegexBuilder::new(&self.expression)
                    .case_insensitive(!is_case_sensitive)
                    .build()
                {
                    regex.replace(parsed_regex);
                }
            }
        }
    }

    pub fn matches(&self, value: &str) -> bool {
        match &self.mode {
            FilterMode::PartialMatch {
                flags,
            } => {
                let mut value = value.to_string();
                let mut expression = self.expression.to_string();
                if !flags.contains(&PartialMatchFlag::CaseSensitive) {
                    expression = expression.to_lowercase();
                    value = value.to_lowercase();
                }

                value.contains(expression.as_str())
            }
            FilterMode::RegexMatch {
                flags: _flags,
                regex,
            } => {
                if let Some(pattern) = regex {
                    pattern.is_match(value)
                } else {
                    false
                }
            }
        }
    }

    pub fn is_regex_mode(&self) -> bool {
        matches!(self.mode, FilterMode::RegexMatch { .. })
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            expression: Default::default(),
            mode: FilterMode::PartialMatch {
                flags: HashSet::new(),
            },
            component_state: Default::default(),
        }
    }
}

pub enum FilterMode {
    PartialMatch {
        flags: HashSet<PartialMatchFlag>,
    },

    // store the regex it can be parsed just one, when the filter changes.
    RegexMatch {
        flags: HashSet<RegexFilterFlag>,
        regex: Option<Regex>,
    },
}

impl Hash for FilterMode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            FilterMode::PartialMatch {
                flags,
            } => {
                "PM".hash(state);
                format!("{:?}", flags).hash(state);
            }
            FilterMode::RegexMatch {
                flags, ..
            } => {
                "RM".hash(state);
                format!("{:?}", flags).hash(state);
            }
        }
    }
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub enum RegexFilterFlag {
    CaseSensitive,
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub enum PartialMatchFlag {
    CaseSensitive,
    // FUTURE WholeWords,
}

impl Filter {}

#[derive(Default, Debug)]
pub struct FilterUiContext {}

#[derive(Debug, Clone)]
pub enum FilterUiCommand {
    ExpressionChanged(String),
    CaseSensitiveButtonClicked(bool),
    RegexButtonClicked(bool),
    ClearExpressionClicked,
}

#[derive(Debug)]
pub enum FilterUiAction {
    ApplyFilter,
}

pub const MARGIN: Margin = Margin::symmetric(4, 2);

impl UiComponent for Filter {
    type UiContext<'context> = FilterUiContext;
    type UiCommand = FilterUiCommand;
    type UiAction = FilterUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.horizontal(|ui| {
            let mut expression = self.expression.clone();

            // combine the TextEdit and button in a single frame.
            egui::Frame::group(&Style::default())
                .inner_margin(MARGIN)
                .outer_margin(egui::Margin::ZERO)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut expression)
                            .hint_text(tr!("filter-expression"))
                            .frame(Frame::NONE),
                    );

                    if ui
                        .add(egui::Button::new("x").frame(false))
                        .clicked()
                    {
                        self.component_state
                            .send(FilterUiCommand::ClearExpressionClicked)
                    };
                });

            if !expression.eq(&self.expression) {
                self.component_state
                    .send(FilterUiCommand::ExpressionChanged(expression))
            }

            ui.add_enabled_ui(self.does_mode_support_case_sensitivity(), |ui| {
                let mut is_case_sensitive = self.is_case_sensitive();
                if ui
                    .toggle_value(&mut is_case_sensitive, "Cc")
                    .changed()
                {
                    self.component_state
                        .send(FilterUiCommand::CaseSensitiveButtonClicked(is_case_sensitive))
                }
            });

            let mut is_regex_mode = self.is_regex_mode();
            if ui
                .toggle_value(&mut is_regex_mode, ".*")
                .changed()
            {
                self.component_state
                    .send(FilterUiCommand::RegexButtonClicked(is_regex_mode))
            }
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("Filter command. command: {:?}", command);
        match command {
            FilterUiCommand::ExpressionChanged(expression) => {
                self.expression = expression;
                self.on_expression_changed();

                Some(FilterUiAction::ApplyFilter)
            }
            FilterUiCommand::ClearExpressionClicked => {
                self.expression.clear();
                self.on_expression_changed();

                Some(FilterUiAction::ApplyFilter)
            }
            FilterUiCommand::CaseSensitiveButtonClicked(is_case_sensitive) => {
                self.set_case_sensitivity(is_case_sensitive);
                self.on_expression_changed();

                Some(FilterUiAction::ApplyFilter)
            }
            FilterUiCommand::RegexButtonClicked(is_regex) => {
                let is_case_sensitive = self.is_case_sensitive();

                match is_regex {
                    true => {
                        self.mode = FilterMode::RegexMatch {
                            flags: HashSet::new(),
                            regex: None,
                        }
                    }
                    false => {
                        self.mode = FilterMode::PartialMatch {
                            flags: HashSet::new(),
                        }
                    }
                }

                self.set_case_sensitivity(is_case_sensitive);

                self.on_expression_changed();

                Some(FilterUiAction::ApplyFilter)
            }
        }
    }
}
