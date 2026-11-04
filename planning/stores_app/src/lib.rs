//! The stores core: the data sources the front ends use, such as the user's parts.
//!
//! A data source is used through this core, not by reading its files, so that it can later be kept somewhere other
//! than a CSV file, for example in a database, and so that one core can serve every front end, and every window of a
//! front end, that uses the same data source.
//!
//! The core is pure: it asks its shell for each store operation, as a [`StoresOperation`] effect, which the shell has
//! the stores service do, and is given the result as an event (`STORE-7`).

// A core does no IO, see `clippy.toml`.
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

use std::collections::BTreeMap;
use std::future::Future;

use crux_core::capability::Operation;
use crux_core::command::NotificationBuilder;
use crux_core::macros::effect;
use crux_core::render::RenderOperation;
pub use crux_core::Core;
use crux_core::{render, App, Command, Request};
pub use stores_protocol::{Part, Parts, PartsSource, PartsViewItem, Revision, StoresOperation, StoresOutput};
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Default)]
pub struct Stores;

#[derive(Default)]
pub struct Model {
    /// Each data source as it was asked for, and the data source the stores service identified it as (`STORE-6`).
    aliases: BTreeMap<PartsSource, PartsSource>,

    /// Each data source, as the stores service identified it.
    sources: BTreeMap<PartsSource, PartsSourceState>,

    error: Option<String>,
}

#[derive(Default)]
struct PartsSourceState {
    /// The parts loaded, in the order the data source has them, kept when a later load is refused.
    parts: Option<Vec<PartsViewItem>>,
    /// The revision the parts were read from (`STORE-8`).
    revision: Option<Revision>,
    /// Whether the parts are being read.
    reading: bool,
    /// Whether the data source changed while its parts were being read, so they're out of date once read (`STORE-9`).
    changed_while_reading: bool,
    /// Whether the core asked for the data source to be watched (`STORE-9`).
    watching: bool,
    /// Why the last load was refused, until one succeeds.
    error: Option<String>,
}

/// What the front ends need to know after each event.
#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug, Clone)]
pub struct StoresViewModel {
    /// Why the last event was refused, if it was.
    pub error: Option<String>,

    /// Each data source the core was asked for, by the source as it was asked for, and as the stores service
    /// identified it, once it has.
    pub parts_sources: BTreeMap<PartsSource, PartsSourceStatus>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PartsSourceStatus {
    /// The data source as the stores service identified it, the one its views have (`STORE-6`).
    pub source: PartsSource,
    pub loaded: bool,
    /// The revision of the parts loaded, if any (`STORE-8`).
    pub revision: Option<Revision>,
    pub reading: bool,
    /// Why the last load was refused, until one succeeds. The parts loaded before, if any, are kept.
    pub error: Option<String>,
}

#[effect]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    PartsView(PartsViewRendererOperation),
    Stores(StoresOperation),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum Event {
    /// Opens the data source: loads its parts, unless they're loaded already, then sends its parts view (`STORE-5`).
    OpenParts { source: PartsSource },

    /// Loads the parts from the data source again, replacing any loaded from it before, so that loading again shows the
    /// data source's current parts, then sends its parts view, so that every view of it can be updated.
    LoadParts { source: PartsSource },

    /// Requests the parts loaded from the data source.
    RequestPartsView { source: PartsSource },

    // internal, the results of the store operations the core asked for.
    /// The data source, as the stores service identified it.
    SourceResolved {
        source: PartsSource,
        intent: Intent,
        output: StoresOutput,
    },
    /// The parts of the data source, as the stores service identified it.
    PartsRead { source: PartsSource, output: StoresOutput },
    /// The data source, as the stores service identified it, may have changed (`STORE-9`).
    SourceChanged { source: PartsSource, output: StoresOutput },
}

/// What the data source was resolved for.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Intent {
    Open,
    Load,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PartsView {
    /// The data source as the stores service identified it (`STORE-6`), which may be written differently from the
    /// event's.
    pub source: PartsSource,
    /// The revision of the data source the parts were read from (`STORE-8`).
    pub revision: Revision,
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

    /// The shell gave the result of a different operation, which is a fault in the shell.
    #[error("Unexpected stores output. output: {output}")]
    UnexpectedOutput { output: String },
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
        let status = |source: &PartsSource| {
            model
                .sources
                .get(source)
                .map(|state| PartsSourceStatus {
                    source: source.clone(),
                    loaded: state.parts.is_some(),
                    revision: state.revision.clone(),
                    reading: state.reading,
                    error: state.error.clone(),
                })
        };

        let mut parts_sources: BTreeMap<PartsSource, PartsSourceStatus> = model
            .sources
            .keys()
            .filter_map(|source| status(source).map(|status| (source.clone(), status)))
            .collect();
        for (alias, source) in model.aliases.iter() {
            if let Some(status) = status(source) {
                parts_sources.insert(alias.clone(), status);
            }
        }

        StoresViewModel {
            error: model.error.clone(),
            parts_sources,
        }
    }
}

impl Stores {
    fn update_inner(event: Event, model: &mut Model) -> Result<Command<Effect, Event>, StoresError> {
        match event {
            Event::OpenParts {
                source,
            } => {
                // already identified and loaded, so there's nothing to ask the stores service.
                if let Some(resolved) = model.aliases.get(&source).cloned() {
                    if Self::is_loaded(model, &resolved) {
                        info!("Parts already loaded. source: '{}'", resolved);
                        return Ok(render::render().and(Self::parts_view(model, resolved)?));
                    }
                }

                Ok(Self::resolve(source, Intent::Open))
            }
            Event::LoadParts {
                source,
            } => Ok(Self::resolve(source, Intent::Load)),
            Event::RequestPartsView {
                source,
            } => {
                let resolved = model
                    .aliases
                    .get(&source)
                    .cloned()
                    .unwrap_or(source);

                Self::parts_view(model, resolved)
            }
            Event::SourceResolved {
                source,
                intent,
                output,
            } => {
                let StoresOutput::SourceResolved {
                    source: resolved,
                } = output
                else {
                    return Err(StoresError::UnexpectedOutput {
                        output: format!("{:?}", output),
                    });
                };
                model
                    .aliases
                    .insert(source, resolved.clone());
                let state = model
                    .sources
                    .entry(resolved.clone())
                    .or_default();

                // `STORE-9`, once for each data source, so that its views are updated whoever changes it.
                let watch = match state.watching {
                    true => Command::done(),
                    false => {
                        state.watching = true;
                        Self::watch(resolved.clone())
                    }
                };

                let command = match intent {
                    Intent::Open if state.parts.is_some() => {
                        info!("Parts already loaded. source: '{}'", resolved);
                        render::render().and(Self::parts_view(model, resolved)?)
                    }
                    // another open is reading them, and its view will be sent to every view of the data source.
                    Intent::Open if state.reading => render::render(),
                    Intent::Open | Intent::Load => {
                        state.reading = true;
                        render::render().and(Self::read(resolved))
                    }
                };

                Ok(command.and(watch))
            }
            Event::SourceChanged {
                source,
                output,
            } => {
                let StoresOutput::Changed = output else {
                    return Err(StoresError::UnexpectedOutput {
                        output: format!("{:?}", output),
                    });
                };
                info!("Data source changed. source: '{}'", source);
                let state = model
                    .sources
                    .entry(source.clone())
                    .or_default();

                if state.reading {
                    // the parts being read may be from before the change, so they're read again once they are.
                    state.changed_while_reading = true;
                    return Ok(render::render());
                }

                state.reading = true;
                Ok(render::render().and(Self::read(source)))
            }
            Event::PartsRead {
                source,
                output,
            } => {
                let state = model
                    .sources
                    .entry(source.clone())
                    .or_default();

                // `STORE-9`, out of date, as the data source changed while they were read, so they're read again.
                if state.changed_while_reading {
                    state.changed_while_reading = false;
                    info!(
                        "Data source changed while being read, reading it again. source: '{}'",
                        source
                    );
                    return Ok(Self::read(source));
                }
                state.reading = false;

                let parts = match output {
                    StoresOutput::PartsRead(Ok(parts)) => parts,
                    StoresOutput::PartsRead(Err(cause)) => {
                        let error = StoresError::UnableToLoadParts {
                            parts_source: source,
                            cause,
                        };
                        // `STORE-3`, the parts loaded before, if any, are kept.
                        state.error = Some(error.to_string());
                        return Err(error);
                    }
                    output => {
                        return Err(StoresError::UnexpectedOutput {
                            output: format!("{:?}", output),
                        });
                    }
                };

                let Parts {
                    revision,
                    parts,
                } = parts;
                state.error = None;

                // `STORE-8`, every view of the data source shows this revision already.
                if state.parts.is_some() && state.revision.as_ref() == Some(&revision) {
                    info!("Parts unchanged. source: '{}', revision: '{}'", source, revision);
                    return Ok(render::render());
                }

                info!(
                    "Loaded parts. source: '{}', part_count: {}, revision: '{}'",
                    source,
                    parts.len(),
                    revision
                );
                state.parts = Some(parts);
                state.revision = Some(revision);

                Ok(render::render().and(Self::parts_view(model, source)?))
            }
        }
    }

    fn is_loaded(model: &Model, source: &PartsSource) -> bool {
        model
            .sources
            .get(source)
            .is_some_and(|state| state.parts.is_some())
    }

    fn resolve(source: PartsSource, intent: Intent) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::ResolveSource {
            source: source.clone(),
        })
        .then_send(move |output| Event::SourceResolved {
            source: source.clone(),
            intent,
            output,
        })
    }

    /// A stream: the shell answers each time the data source may have changed, for as long as it can watch it.
    fn watch(source: PartsSource) -> Command<Effect, Event> {
        Command::stream_from_shell(StoresOperation::WatchSource {
            source: source.clone(),
        })
        .then_send(move |output| Event::SourceChanged {
            source: source.clone(),
            output,
        })
    }

    fn read(source: PartsSource) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::ReadParts {
            source: source.clone(),
        })
        .then_send(move |output| Event::PartsRead {
            source: source.clone(),
            output,
        })
    }

    fn parts_view(model: &Model, source: PartsSource) -> Result<Command<Effect, Event>, StoresError> {
        let (parts, revision) = model
            .sources
            .get(&source)
            .and_then(|state| {
                state
                    .parts
                    .as_ref()
                    .zip(state.revision.as_ref())
            })
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
            revision: revision.clone(),
            field_names,
            parts: parts.clone(),
        };

        Ok(parts_view_builder(view).into())
    }
}
