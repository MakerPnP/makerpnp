//! The stores core: the data sources the front ends use, such as the user's parts.
//!
//! A data source is used through this core, not by reading its files, so that it can later be kept somewhere other
//! than a CSV file, for example in a database, and so that one core can serve every front end, and every window of a
//! front end, that uses the same data source.

use std::collections::BTreeMap;
use std::future::Future;

use crux_core::capability::Operation;
use crux_core::command::NotificationBuilder;
use crux_core::macros::effect;
use crux_core::render::RenderOperation;
pub use crux_core::Core;
use crux_core::{render, App, Command, Request};
pub use pnp::part::Part;
pub use stores::parts::PartsSource;
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Default)]
pub struct Stores;

#[derive(Default)]
pub struct Model {
    /// The parts of each parts data source loaded, in the order the data source has them.
    parts: BTreeMap<PartsSource, Vec<PartsViewItem>>,

    error: Option<String>,
}

/// What the front ends need to know after each event.
#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug, Clone)]
pub struct StoresViewModel {
    /// Why the last event was refused, if it was.
    pub error: Option<String>,
}

#[effect]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    PartsView(PartsViewRendererOperation),
}

/// Each event's data source is matched by its canonical path, so one file is one data source however its path is
/// written (`STORE-6`).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum Event {
    /// Opens the data source: loads its parts, unless they're loaded already, then sends its parts view (`STORE-5`).
    OpenParts { source: PartsSource },

    /// Loads the parts from the data source again, replacing any loaded from it before, so that loading again shows the
    /// data source's current parts, then sends its parts view, so that every view of it can be updated.
    LoadParts { source: PartsSource },

    /// Requests the parts loaded from the data source.
    RequestPartsView { source: PartsSource },
}

/// The data source, with its file's canonical path, if it has a file that exists, otherwise as it is (`STORE-6`).
///
/// For example `d:\parts\Parts.csv` and `D:\parts\..\parts\parts.csv` are the same file on Windows, so the same data
/// source.
pub fn canonical_parts_source(source: &PartsSource) -> PartsSource {
    match source {
        PartsSource::File(path) => dunce::canonicalize(path)
            .map(PartsSource::File)
            .unwrap_or_else(|_| source.clone()),
        _ => source.clone(),
    }
}

/// A part, and the data source's other values for it, for example a supplier's part code.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PartsViewItem {
    pub part: Part,
    /// The data source's other values for the part, by name, for example `"LCSC"`.
    pub fields: BTreeMap<String, String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PartsView {
    /// The canonical data source (`STORE-6`), which may be written differently from the event's.
    pub source: PartsSource,
    /// The names of the parts' other values, in order, for example `["LCSC"]`.
    pub field_names: Vec<String>,
    /// The parts, in the order the data source has them.
    pub parts: Vec<PartsViewItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum PartsViewRendererOperation {
    View { view: PartsView },
}

impl Operation for PartsViewRendererOperation {
    type Output = ();
}

fn parts_view_builder<Effect, Event>(view: PartsView) -> NotificationBuilder<Effect, Event, impl Future<Output = ()>>
where
    Effect: From<Request<PartsViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    Command::notify_shell(PartsViewRendererOperation::View {
        view,
    })
}

#[derive(Error, Debug)]
pub enum StoresError {
    #[error("Unable to load the parts. source: '{parts_source}', cause: {cause}")]
    UnableToLoadParts { parts_source: PartsSource, cause: String },

    #[error("The parts are not loaded. source: '{parts_source}'")]
    PartsNotLoaded { parts_source: PartsSource },
}

impl App for Stores {
    type Event = Event;
    type Model = Model;
    type ViewModel = StoresViewModel;
    type Effect = Effect;

    fn update(&self, event: Self::Event, model: &mut Self::Model) -> Command<Self::Effect, Self::Event> {
        // `LOG-1`
        debug!("Received event. event: {:?}", event);

        match Self::update_inner(event, model) {
            Ok(command) => {
                model.error = None;
                command
            }
            Err(error) => {
                // `LOG-2`
                warn!("Event failed. error: {:?}", error);
                model.error = Some(error.to_string());
                render::render()
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        StoresViewModel {
            error: model.error.clone(),
        }
    }
}

impl Stores {
    fn update_inner(event: Event, model: &mut Model) -> Result<Command<Effect, Event>, StoresError> {
        match event {
            Event::OpenParts {
                source,
            } => {
                let source = canonical_parts_source(&source);
                if model.parts.contains_key(&source) {
                    info!("Parts already loaded. source: '{}'", source);
                } else {
                    Self::load_parts(model, &source)?;
                }

                Ok(render::render().and(Self::parts_view(model, source)?))
            }
            Event::LoadParts {
                source,
            } => {
                let source = canonical_parts_source(&source);
                Self::load_parts(model, &source)?;

                Ok(render::render().and(Self::parts_view(model, source)?))
            }
            Event::RequestPartsView {
                source,
            } => {
                let source = canonical_parts_source(&source);

                Self::parts_view(model, source)
            }
        }
    }

    /// Loads the parts, replacing those loaded before only if they load.
    fn load_parts(model: &mut Model, source: &PartsSource) -> Result<(), StoresError> {
        let parts = stores::parts::load_parts(source).map_err(|error| StoresError::UnableToLoadParts {
            parts_source: source.clone(),
            // `{:#}` includes each cause, for example which header is missing.
            cause: format!("{:#}", error),
        })?;

        let parts: Vec<PartsViewItem> = parts
            .into_iter()
            .map(|(part, fields)| PartsViewItem {
                part,
                fields: fields.into_iter().collect(),
            })
            .collect();

        info!("Loaded parts. source: '{}', part_count: {}", source, parts.len());
        model
            .parts
            .insert(source.clone(), parts);

        Ok(())
    }

    fn parts_view(model: &Model, source: PartsSource) -> Result<Command<Effect, Event>, StoresError> {
        let parts = model
            .parts
            .get(&source)
            .ok_or_else(|| StoresError::PartsNotLoaded {
                parts_source: source.clone(),
            })?;

        let mut field_names: Vec<String> = parts
            .iter()
            .flat_map(|item| item.fields.keys().cloned())
            .collect();
        field_names.sort();
        field_names.dedup();

        let view = PartsView {
            source,
            field_names,
            parts: parts.clone(),
        };

        Ok(parts_view_builder(view).into())
    }
}
