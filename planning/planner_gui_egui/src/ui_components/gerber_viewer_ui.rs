use std::collections::HashMap;
use std::fs::File;
use std::hash::Hash;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;

use derivative::Derivative;
use eda_units::eda_units::dimension_unit::{DimensionUnit, DimensionUnitPoint2Ext, Point2DimensionUnitExt};
use eda_units::eda_units::unit_system::UnitSystem;
use eframe::emath::{Align2, Rect, Vec2};
use eframe::epaint::{Color32, FontId};
use egui::{Pos2, Stroke, Ui};
use gerber_viewer::gerber_parser::{GerberDoc, ParseError, parse};
use gerber_viewer::gerber_types::Command;
use gerber_viewer::{
    BoundingBox, GerberLayer, GerberRenderer, GerberTransform, RenderConfiguration, ToPosition, UiState, ViewState,
    draw_crosshair, generate_pastel_color,
};
use indexmap::IndexMap;
use indexmap::map::Entry;
use nalgebra::{Point2, Vector2};
use planner_app::{
    DesignIndex, GerberFileFunction, PanelSizing, PcbAssemblyOrientation, PcbOverview, PcbSide, PcbUnitIndex,
    PlacementPositionUnit,
};
use thiserror::Error;
use tracing::{debug, error, info, trace};

use crate::sync::Value;
use crate::ui_component::{ComponentState, UiComponent};

pub type LayersMap =
    IndexMap<(Option<PathBuf>, Option<GerberFileFunction>), (LayerViewState, GerberLayer, Option<GerberDoc>)>;

const INITIAL_GERBER_AREA_PERCENT: f32 = 0.95;

/// The arguments required to create a new instance or reference an existing instance of the UI component.
/// Value object
#[derive(
    Default,
    Debug,
    serde::Deserialize,
    serde::Serialize,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash
)]
pub struct GerberViewerUiInstanceArgs {
    pub mode: GerberViewerMode,
    // TODO a way to select a specific set of gerbers, e.g. profile + top paste + top solder + top silk
    //      using PCB side will show copper layers, which isn't very useful for PnP
    //      perhaps Vec<GerberFileFunction> ?
    /// Which sides to show, gerber files without a side, like Profile, will also be shown.
    pub pcb_side: Option<PcbSide>,
}

#[derive(
    Derivative,
    Debug,
    serde::Deserialize,
    serde::Serialize,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash
)]
#[derivative(Default)]
pub enum GerberViewerMode {
    #[derivative(Default)]
    Panel,
    Design(DesignIndex),
}

#[derive(Debug)]
pub struct GerberViewerUi {
    args: GerberViewerUiInstanceArgs,

    assembly_orientation: Option<PcbAssemblyOrientation>,
    panel_sizing: Option<PanelSizing>,
    unit_map: Option<HashMap<PcbUnitIndex, DesignIndex>>,

    placement_marker: Option<PlacementPositionUnit>,

    gerber_state: Value<GerberViewState>,
    gerber_ui_state: Value<UiState>,

    pub component: ComponentState<GerberViewerUiCommand>,
}

impl GerberViewerUi {
    pub fn new(args: GerberViewerUiInstanceArgs) -> Self {
        let layers = Value::new(IndexMap::new());

        Self {
            args,

            assembly_orientation: None,
            panel_sizing: None,
            unit_map: None,

            placement_marker: None,

            gerber_state: Value::new(GerberViewState::new(layers.clone())),
            gerber_ui_state: Value::default(),

            component: Default::default(),
        }
    }

    pub fn layers(&self) -> Value<LayersMap> {
        self.gerber_state
            .lock()
            .unwrap()
            .layers
            .clone()
    }

    pub fn clear_layers(&mut self) {
        self.gerber_state
            .lock()
            .unwrap()
            .update_layers(IndexMap::new());
    }

    pub fn set_panel_sizing(&mut self, new_panel_sizing: PanelSizing) {
        self.panel_sizing = Some(new_panel_sizing);
    }

    pub fn set_assembly_orientation(&mut self, new_assembly_orientation: PcbAssemblyOrientation) {
        self.assembly_orientation = Some(new_assembly_orientation);
    }

    pub fn set_unit_map(&mut self, new_unit_map: HashMap<PcbUnitIndex, DesignIndex>) {
        self.unit_map = Some(new_unit_map);
    }

    pub fn add_layer(&mut self, function: Option<GerberFileFunction>, commands: Vec<Command>) {
        let mut gerber_state = self.gerber_state.lock().unwrap();

        let (state, layer) = Self::build_gerber_layer_from_commands(0, commands);

        gerber_state.add_layer(None, function, state, layer, None);
    }

    pub fn request_center_view(&mut self) {
        let mut gerber_state = self.gerber_state.lock().unwrap();
        gerber_state.request_center_view();
    }

    pub fn update_layers_from_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        let (new_orientation, new_pcb_gerbers, new_design_gerbers, unit_map) = (
            pcb_overview.orientation,
            pcb_overview.pcb_gerbers,
            pcb_overview.design_gerbers,
            pcb_overview.unit_map,
        );

        self.assembly_orientation = Some(new_orientation);
        self.unit_map = Some(unit_map);

        let gerber_items = match &self.args.mode {
            GerberViewerMode::Panel => new_pcb_gerbers,
            GerberViewerMode::Design(design_index) => new_design_gerbers[*design_index].clone(),
        };

        let gerber_items = gerber_items
            .iter()
            .filter(|gerber| match self.args.pcb_side {
                // if no PCB side filter is specified, include this item
                None => true,
                Some(pcb_side) => {
                    if let Some(function) = gerber.function {
                        let gerber_side = function.pcb_side();
                        match gerber_side {
                            // if the item has no side, include it.
                            None => true,
                            // if a PCB side is specified, include this item if it's side matches
                            Some(gerber_side) => gerber_side == pcb_side,
                        }
                    } else {
                        // if there is no function, exclude it
                        false
                    }
                }
            })
            .collect::<Vec<_>>();

        // The new list of gerber items may contain fewer, more or different entries and/or the same entries in a different
        // order.  Only to reparse files that need reparsing, use the layer order defined by the gerber items collection.

        let mut gerber_state = self.gerber_state.lock().unwrap();

        // FUTURE move this, e.g. `GerberLayerState::sync_layers` and give it the `FBuild` and `FKey` closures as arguments.
        // FUTURE load each layer in a background thead, don't block the UI thread.
        {
            let mut layers = gerber_state
                .layers
                .lock()
                .unwrap()
                .split_off(0);

            let errors = sync_indexmap(
                &mut layers,
                &gerber_items,
                |index, (path, _name), _content| {
                    Self::build_gerber_layer_from_file(index, path.as_ref().unwrap()).map(
                        |(mut layer_view_state, layer, gerber_doc)| {
                            if matches!(self.args.pcb_side, Some(PcbSide::Bottom)) {
                                layer_view_state.transform.mirroring.x = true
                            }

                            (layer_view_state, layer, Some(gerber_doc))
                        },
                    )
                },
                |content| (Some(content.path.clone()), content.function),
                |_existing_entry, _gerber_item| true,
            );

            for (index, error) in errors {
                error!(
                    "Error adding gerber layer. path: {:?}, error: {}",
                    gerber_items[index].path, error
                );
            }

            gerber_state.update_layers(layers);
            gerber_state.request_center_view();
        }

        /// Synchronizes an index map with a source-of-truth vector using a fallible builder.
        /// Removes old items if not present in the vector.
        /// index map entry ordering will match the ordering of items in the vector.
        pub fn sync_indexmap<K, T, S, E, FBuild, FKey, FReuse>(
            map: &mut IndexMap<K, T>,
            source: &[S],
            build_fn: FBuild,
            extract_key: FKey,
            reuse_check: FReuse,
        ) -> Vec<(usize, E)>
        where
            K: Eq + Hash + Clone,
            // build the new entry
            FBuild: Fn(usize, &K, &S) -> Result<T, E>,
            // return the key to use for the new entry
            FKey: Fn(&S) -> K,
            // return true to keep the existing entry, false to replace it with the new one
            FReuse: Fn(&T, &S) -> bool,
        {
            let mut new_map: IndexMap<K, T> = IndexMap::with_capacity(source.len());
            let mut errors = Vec::new();

            for (i, item) in source.iter().enumerate() {
                let key = extract_key(item);
                match map.entry(key.clone()) {
                    Entry::Occupied(entry) if reuse_check(entry.get(), &item) => {
                        let existing_entry = entry.shift_remove();
                        new_map.insert(key, existing_entry);
                    }
                    _ => match build_fn(new_map.len(), &key, item) {
                        Ok(new_val) => {
                            new_map.insert(key, new_val);
                        }
                        Err(err) => {
                            errors.push((i, err));
                        }
                    },
                }
            }

            *map = new_map;

            errors
        }
    }

    fn build_gerber_layer_from_file(
        index: usize,
        path: &PathBuf,
    ) -> Result<(LayerViewState, GerberLayer, GerberDoc), GerberViewerUiError> {
        let (gerber_doc, commands) = Self::parse_gerber(path)?;
        let (state, layer) = Self::build_gerber_layer_from_commands(index, commands);

        Ok((state, layer, gerber_doc))
    }

    fn build_gerber_layer_from_commands(index: usize, commands: Vec<Command>) -> (LayerViewState, GerberLayer) {
        let color = generate_pastel_color(index as u64);

        let layer = GerberLayer::new(commands);
        let layer_view_state = LayerViewState::new(color);

        (layer_view_state, layer)
    }

    fn parse_gerber(
        path: &PathBuf,
    ) -> Result<(GerberDoc, Vec<gerber_viewer::gerber_types::Command>), GerberViewerUiError> {
        let file = File::open(path.clone())
            .inspect_err(|error| {
                let message = format!(
                    "Error parsing gerber file: {}, cause: {}",
                    path.to_str().unwrap(),
                    error
                );
                error!("{}", message);
            })
            .map_err(GerberViewerUiError::IoError)?;

        let reader = BufReader::new(file);

        let gerber_doc: GerberDoc =
            parse(reader).map_err(|(_partial_doc, error)| GerberViewerUiError::ParserError(error))?;

        let message = format!("Gerber file parsed successfully. path: {}", path.to_str().unwrap());
        info!("{}", message);

        let commands = gerber_doc
            .commands
            .iter()
            .filter_map(|c| match c {
                Ok(command) => Some(command.clone()),
                Err(_) => None,
            })
            .collect::<Vec<Command>>();

        Ok((gerber_doc, commands))
    }

    /// X and Y are in dimension units.
    pub fn locate_view(&mut self, mut point: Point2<DimensionUnit>) {
        if matches!(self.args.pcb_side, Some(PcbSide::Bottom)) {
            point.x = -point.x;
        }

        let ui_state = self.gerber_ui_state.lock().unwrap();
        let center_screen_pos = ui_state.center_screen_pos;

        let mut gerber_state = self.gerber_state.lock().unwrap();
        gerber_state.locate_view(point, center_screen_pos);
    }

    /// rotation is in degrees.
    pub fn show_placement_marker(&mut self, position: PlacementPositionUnit) {
        self.placement_marker = Some(position);
    }

    pub fn clear_placement_marker(&mut self) {
        self.placement_marker = None;
    }
}

#[derive(Debug)]
struct GerberViewState {
    view: ViewState,
    needs_view_centering: bool,
    needs_bbox_update: bool,
    bounding_box: BoundingBox,
    render_configuration: RenderConfiguration,
    layers: Value<LayersMap>,
    transform: GerberTransform,
    target_unit_system: UnitSystem,
}

impl Default for GerberViewState {
    fn default() -> Self {
        Self {
            view: Default::default(),
            needs_view_centering: true,
            needs_bbox_update: true,
            bounding_box: BoundingBox::default(),
            render_configuration: RenderConfiguration::default(),
            layers: Default::default(),
            transform: GerberTransform::default(),
            target_unit_system: UnitSystem::Millimeters,
        }
    }
}

impl GerberViewState {
    pub fn new(layers: Value<LayersMap>) -> Self {
        Self {
            layers,
            ..Default::default()
        }
    }

    pub fn add_layer(
        &mut self,
        path: Option<PathBuf>,
        function: Option<GerberFileFunction>,
        layer_view_state: LayerViewState,
        layer: GerberLayer,
        gerber_doc: Option<GerberDoc>,
    ) {
        self.layers
            .lock()
            .unwrap()
            .insert((path, function), (layer_view_state, layer, gerber_doc));
        self.update_unit_system();
        self.update_bbox_from_layers();
    }

    /// Must be called after manipulating `self.layers`
    fn update_unit_system(&mut self) {
        let layers = self.layers.lock().unwrap();
        if let Some(((_, _), (_state, _layer, gerber_doc))) = layers.first() {
            self.target_unit_system = gerber_doc
                .as_ref()
                .map_or(UnitSystem::Millimeters, |doc| {
                    let first_layer_gerber_unit_systems = &doc.units;
                    UnitSystem::from_gerber_unit(first_layer_gerber_unit_systems)
                });
            info!("target_unit_system: {:?}", self.target_unit_system);
        }
    }

    pub fn update_layers(&mut self, layers: LayersMap) {
        self.layers = Value::new(layers);
        self.update_unit_system();
        self.update_bbox_from_layers();
    }

    fn update_bbox_from_layers(&mut self) {
        let mut bbox = BoundingBox::default();

        for (layer_index, ((_path, _name), (layer_view_state, layer, _))) in self
            .layers
            .lock()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, (_path, (_, layer, _)))| !layer.is_empty())
        {
            let layer_bbox = &layer.bounding_box();

            let layer_transform = layer_view_state
                .transform
                .combine(&self.transform);

            let layer_bbox = layer_bbox.apply_transform(&layer_transform);

            debug!("layer bbox: {:?}", layer_bbox);
            bbox.min.x = f64::min(bbox.min.x, layer_bbox.min.x);
            bbox.min.y = f64::min(bbox.min.y, layer_bbox.min.y);
            bbox.max.x = f64::max(bbox.max.x, layer_bbox.max.x);
            bbox.max.y = f64::max(bbox.max.y, layer_bbox.max.y);
            debug!("view bbox after layer. layer: {}, bbox: {:?}", layer_index, bbox);
        }

        self.bounding_box = bbox;
        self.needs_bbox_update = false;
    }

    pub fn request_center_view(&mut self) {
        self.needs_view_centering = true;
    }

    fn reset_view(&mut self, viewport: Rect) {
        self.update_bbox_from_layers();
        self.view
            .fit_view(viewport, &self.bounding_box, INITIAL_GERBER_AREA_PERCENT);
        self.needs_view_centering = false;
    }

    fn locate_view(&mut self, point: Point2<DimensionUnit>, center_screen_pos: Pos2) {
        trace!("locate view. x: {}, y: {}", point.x, point.y);
        let gerber_coords: Point2<DimensionUnit> = point.in_unit_system(self.target_unit_system);
        trace!("gerber_coords: {:?}", gerber_coords);

        let (x, y) = (gerber_coords.x.value_f64(), gerber_coords.y.value_f64());

        self.view.translation = Vec2::new(
            center_screen_pos.x - (x as f32 * self.view.scale),
            center_screen_pos.y + (y as f32 * self.view.scale),
        );
        trace!("view translation (after): {:?}", self.view.translation);
    }
}

#[derive(Error, Debug)]
enum GerberViewerUiError {
    #[error("IO Error. cause: {0:?}")]
    IoError(io::Error),

    #[error("Parser error. cause: {0:?}")]
    ParserError(ParseError),
}

#[derive(Debug, Clone)]
pub struct LayerViewState {
    color: Color32,
    transform: GerberTransform,
}

impl LayerViewState {
    fn new(color: Color32) -> Self {
        Self {
            color,
            transform: GerberTransform::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum GerberViewerUiCommand {
    None,
    LocateView(Point2<DimensionUnit>),
    ShowPlacementMarker(PlacementPositionUnit),
}

#[derive(Debug, Clone)]
pub enum GerberViewerUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct GerberViewerUiContext {}

impl UiComponent for GerberViewerUi {
    type UiContext<'context> = GerberViewerUiContext;
    type UiCommand = GerberViewerUiCommand;
    type UiAction = GerberViewerUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let mut state = self.gerber_state.lock().unwrap();

        let response = ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::drag());
        let viewport = response.rect;

        if state.needs_bbox_update {
            state.update_bbox_from_layers();
        }

        if state.needs_view_centering {
            state.reset_view(viewport);
        }

        let mut ui_state = self.gerber_ui_state.lock().unwrap();
        ui_state.update(ui, &viewport, &response, &mut state.view);

        let painter = ui.painter().with_clip_rect(viewport);

        let mut request_draw_unit_unit_numbers = matches!(self.args.mode, GerberViewerMode::Panel);
        let mut request_draw_placement_marker = self.placement_marker.is_some();

        let layers = state.layers.lock().unwrap();
        let layer_count = layers.len();
        for (index, (_path, (layer_view_state, layer, doc))) in layers.iter().enumerate() {
            let is_last_layer = index == layer_count - 1;

            let layer_transform = layer_view_state
                .transform
                .combine(&state.transform);

            let units = doc.as_ref().and_then(|doc| doc.units);
            let doc_unit_system = UnitSystem::from_gerber_unit(&units);

            let renderer = GerberRenderer::new(&state.render_configuration, state.view, &layer_transform, layer);
            renderer.paint_layer(&painter, layer_view_state.color);

            if is_last_layer {
                // draw on top of the last layer

                if request_draw_unit_unit_numbers {
                    request_draw_unit_unit_numbers = false;

                    // Draw unit numbers
                    if let (Some(panel_sizing), Some(unit_map)) = (&self.panel_sizing, &self.unit_map) {
                        for (unit_index, unit_positioning) in panel_sizing
                            .pcb_unit_positionings
                            .iter()
                            .enumerate()
                        {
                            let unit_number = unit_index as u32 + 1;

                            let Some(design_index) = unit_map
                                .get(&(unit_index as PcbUnitIndex))
                                .cloned()
                            else {
                                continue;
                            };
                            let design_sizing = &panel_sizing.design_sizings[design_index];

                            let unit_center = unit_positioning.offset + (design_sizing.size / 2.0);
                            let unit_center_mm = unit_center
                                .to_position()
                                .to_dimension_unit(UnitSystem::Millimeters);
                            let position_doc = unit_center_mm.to_point2(doc_unit_system);

                            let offset_screen_coords = renderer.gerber_to_screen_coordinates(&position_doc);

                            painter.text(
                                offset_screen_coords,
                                Align2::CENTER_CENTER,
                                format!("{}", unit_number),
                                FontId::monospace(32.0),
                                layer_view_state.color.additive(),
                            );
                        }
                    }
                }

                if request_draw_placement_marker {
                    request_draw_placement_marker = false;
                    if let Some(placement_marker_position) = &self.placement_marker {
                        // a line vector in Millimeters, a rotation of 0 means straight up, gerber coords are positive up.
                        let vector_mm = Vector2::new(0.0_f64, 0.5_f64);

                        let rotation = placement_marker_position
                            .rotation
                            .to_radians_f64();

                        // TODO discover and use the correct nalgebra rotation API to rotate the vector instead of this.
                        let rotated_vector_mm = Vector2::new(
                            vector_mm.x * rotation.cos() - vector_mm.y * rotation.sin(),
                            vector_mm.x * rotation.sin() + vector_mm.y * rotation.cos(),
                        );

                        let start_position_mm = placement_marker_position
                            .coords
                            .to_point2(UnitSystem::Millimeters);
                        let end_position_mm = start_position_mm + rotated_vector_mm;

                        let start_position_doc = start_position_mm
                            .to_dimension_unit(UnitSystem::Millimeters)
                            .to_point2(doc_unit_system);
                        let end_position_doc = end_position_mm
                            .to_dimension_unit(UnitSystem::Millimeters)
                            .to_point2(doc_unit_system);

                        let screen_start_position = renderer.gerber_to_screen_coordinates(&start_position_doc);
                        let screen_end_position = renderer.gerber_to_screen_coordinates(&end_position_doc);

                        let stroke = Stroke::new(4.0, Color32::RED);
                        painter.line_segment([screen_start_position, screen_end_position], stroke);

                        let stroke = Stroke::new(2.0, Color32::RED);
                        let radius = screen_start_position.distance(screen_end_position);
                        painter.circle_stroke(screen_start_position, radius, stroke);
                    }
                }
            }
        }

        // Draw origin crosshair
        draw_crosshair(&painter, ui_state.origin_screen_pos, Color32::BLUE);
        draw_crosshair(&painter, ui_state.center_screen_pos, Color32::LIGHT_GRAY);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            GerberViewerUiCommand::None => Some(GerberViewerUiAction::None),
            GerberViewerUiCommand::LocateView(point) => {
                self.locate_view(point);

                None
            }
            GerberViewerUiCommand::ShowPlacementMarker(position) => {
                self.show_placement_marker(position);

                None
            }
        }
    }
}
