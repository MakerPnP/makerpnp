//! The stores core, which has the data sources, such as the user's parts.
//!
//! The application has one, shared by every tab, so that a data source used by more than one tab is loaded once, and
//! so that each tab sees the same data (`PARTSRC-5`).

use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use stores_app::{Core, Effect, Event, PartsSource, PartsView, PartsViewRendererOperation, Stores};

pub struct StoresCoreService {
    core: Core<Stores>,

    /// The latest view of each data source's parts, from the core, shared by every tab that shows them, so that the
    /// tabs don't each keep a copy, and so that each tab shows a data source's parts as soon as they change.
    parts_views: BTreeMap<PartsSource, Arc<PartsView>>,
}

impl Debug for StoresCoreService {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoresCoreService")
            .finish()
    }
}

/// Why the core refused an event, as the core says it, for the user.
#[derive(Debug, Clone, PartialEq)]
pub struct StoresError(pub String);

impl Default for StoresCoreService {
    fn default() -> Self {
        Self::new()
    }
}

impl StoresCoreService {
    pub fn new() -> Self {
        Self {
            core: Core::new(),
            parts_views: Default::default(),
        }
    }

    /// Processes an event, keeping the views the core sends, so that every tab showing their data source shows them.
    pub fn process(&mut self, event: Event) -> Result<(), StoresError> {
        for effect in self.core.process_event(event) {
            match effect {
                Effect::Render(_) => {
                    if let Some(error) = self.core.view().error {
                        return Err(StoresError(error));
                    }
                }
                Effect::PartsView(request) => {
                    let PartsViewRendererOperation::View {
                        view,
                    } = request.operation;
                    self.parts_views
                        .insert(view.source.clone(), Arc::new(view));
                }
            }
        }

        Ok(())
    }

    /// The latest view of the data source's parts, if they're loaded. The source is the canonical one, as the views
    /// have (`STORE-6`).
    pub fn parts_view(&self, source: &PartsSource) -> Option<Arc<PartsView>> {
        self.parts_views.get(source).cloned()
    }
}
