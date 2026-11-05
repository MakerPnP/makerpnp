//! The Variant Builder's core: builds an assembly variant's placements, and its BOM, from a design's EDA placements.
//!
//! It's pure: every data source it reads, and every file it writes, is read and written by the stores service, which
//! its shell asks, so it's tested with no files (`STORE-7`, `STORE-14`).

// A core does no IO, see `clippy.toml`.
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use anyhow::Error;
pub use assembly::assembly_variant::AssemblyVariant;
use assembly::AssemblyVariantProcessor;
use crux_core::command::CommandContext;
use crux_core::macros::effect;
use crux_core::render::RenderOperation;
pub use crux_core::Core;
use crux_core::{render, App, Command};
use csv::QuoteStyle;
use eda::placement::{EdaPlacement, EdaPlacementField};
use eda::substitution::{
    EdaSubstitutionResult, EdaSubstitutionRule, EdaSubstitutionRuleClassification, EdaSubstitutor,
};
pub use eda::EdaTool;
use part_mapper::{PartMapper, PartMapperError, PartMappingError, PartMappingResult, PlacementPartMappingResult};
pub use pnp::load_out::LoadOutItem;
pub use pnp::part::Part;
pub use pnp::placement::RefDes;
use serde_with::serde_as;
pub use stores::assembly_rules::AssemblyRuleSource;
use stores::bom::JLCPCBBOMRecord;
pub use stores::eda_placements::EdaPlacementsSource;
pub use stores::load_out::LoadOutSource;
pub use stores::part_mappings::PartMappingsSource;
pub use stores::parts::PartsSource;
use stores::placements::PlacementRecord;
pub use stores::placements::PlacementsSource;
pub use stores::substitutions::EdaSubstitutionsSource;
use stores::{assembly_rules, part_mappings, substitutions};
pub use stores_protocol::{
    Document, Parts, PartsViewItem, Records, RecordsKind, RecordsOf, Revision, Source, StoresOperation, StoresOutput,
};
use termtree::Tree;
use thiserror::Error;
use tracing::Level;
use tracing::{debug, error, info, trace, warn};
pub use variant_builders::{
    Variant, VariantBuilderChange, VariantBuilderError, VariantBuilderFile, VariantBuilderView,
};

extern crate serde_regex;

pub mod variant_builders;

type Context = CommandContext<Effect, Event>;

#[derive(Default)]
pub struct VariantBuilder;

#[derive(Default)]
pub struct Model {
    error: Option<String>,
    /// The open variant builders, by path (`VARB-3`).
    variant_builders: BTreeMap<PathBuf, VariantBuilderView>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct OperationViewModel {
    /// Why the last event failed, if it did.
    pub error: Option<String>,
    /// The open variant builders, by path (`VARB-3`).
    pub variant_builders: BTreeMap<PathBuf, VariantBuilderView>,
}

#[effect]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    /// A store operation, done by the stores service (`STORE-7`).
    Stores(StoresOperation),
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum Event {
    None,
    /// Creates a variant builder's file, and opens it, refusing to replace one unless asked to (`VARB-2`).
    Create {
        path: PathBuf,
        overwrite: bool,
    },
    /// Opens a variant builder, reading its file (`VARB-3`).
    Open {
        path: PathBuf,
    },
    /// Makes the changes to a variant builder's file (`VARB-4`).
    Configure {
        path: PathBuf,
        changes: Vec<VariantBuilderChange>,
    },
    /// Builds a variant builder's variant (`VARB-5`).
    Build {
        path: PathBuf,
    },
    /// A variant builder's file was written, as it was created or configured, or read, as it was opened, or why not.
    Opened {
        path: PathBuf,
        /// What was done, for the log.
        done: Done,
        result: Result<VariantBuilderView, VariantBuilderError>,
    },
    /// The variant builder's variant was built, and its files written, or why not.
    Built {
        path: PathBuf,
        result: Result<(), VariantBuilderError>,
    },
    //
    // Views
    //
}

/// What was done to a variant builder, see [`Event::Opened`].
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Done {
    Created,
    Opened,
    Configured { change_count: usize },
}

/// What [`Event::Build`] builds, from what.
struct BuildArgs {
    eda_tool: EdaTool,
    placements: EdaPlacementsSource,
    assembly_variant: AssemblyVariant,
    parts: PartsSource,
    part_mappings: PartMappingsSource,
    substitutions: Vec<EdaSubstitutionsSource>,
    load_out: Option<LoadOutSource>,
    assembly_rules: Option<AssemblyRuleSource>,
    output: String,
    output_bom: Option<String>,
    ref_des_exclude_list: Vec<String>,
    ref_des_disable_list: Vec<String>,
}

/// The records of each data source, as read by the stores service, in the order they're read.
struct Inputs {
    eda_placements: Vec<EdaPlacement>,
    substitutions: Vec<(EdaSubstitutionsSource, Vec<BTreeMap<String, String>>)>,
    parts: Vec<PartsViewItem>,
    part_mappings: Vec<BTreeMap<String, String>>,
    load_out_items: Vec<LoadOutItem>,
    assembly_rules: Vec<BTreeMap<String, String>>,
}

/// The contents of each file to write.
struct Outputs {
    placements: String,
    bom: Option<String>,
}

impl App for VariantBuilder {
    type Event = Event;
    type Model = Model;
    type ViewModel = OperationViewModel;
    type Effect = Effect;

    fn update(&self, event: Self::Event, model: &mut Self::Model) -> Command<Self::Effect, Self::Event> {
        // `VARB-6`, `LOG-1`
        debug!("Received event. event: {:?}", event);
        match event {
            Event::None => render::render(),
            Event::Create {
                path,
                overwrite,
            } => {
                model.error = None;
                Command::new(|ctx| async move {
                    let result = variant_builders::create(&ctx, &path, overwrite).await;
                    ctx.send_event(Event::Opened {
                        path,
                        done: Done::Created,
                        result,
                    });
                })
            }
            Event::Open {
                path,
            } => {
                model.error = None;
                Command::new(|ctx| async move {
                    let result = variant_builders::open(&ctx, &path).await;
                    ctx.send_event(Event::Opened {
                        path,
                        done: Done::Opened,
                        result,
                    });
                })
            }
            Event::Configure {
                path,
                changes,
            } => {
                model.error = None;
                let done = Done::Configured {
                    change_count: changes.len(),
                };
                Command::new(|ctx| async move {
                    let result = variant_builders::configure(&ctx, &path, changes).await;
                    ctx.send_event(Event::Opened {
                        path,
                        done,
                        result,
                    });
                })
            }
            Event::Opened {
                path,
                done,
                result,
            } => {
                match result {
                    Ok(view) => {
                        match done {
                            Done::Created => info!("Created variant builder. path: '{}'", path.display()),
                            Done::Opened => info!(
                                "Opened variant builder. path: '{}', revision: '{}'",
                                path.display(),
                                view.revision.0
                            ),
                            Done::Configured {
                                change_count,
                            } => info!(
                                "Configured variant builder. path: '{}', change_count: {}",
                                path.display(),
                                change_count
                            ),
                        }
                        // `VARB-4`, configuring one that isn't open doesn't open it.
                        let configured = matches!(done, Done::Configured { .. });
                        if !configured
                            || model
                                .variant_builders
                                .contains_key(&path)
                        {
                            model
                                .variant_builders
                                .insert(path, view);
                        }
                    }
                    Err(error) => fail(model, error),
                }
                render::render()
            }
            Event::Build {
                path,
            } => {
                model.error = None;
                Command::new(|ctx| async move {
                    let result = variant_builders::build(&ctx, &path).await;
                    ctx.send_event(Event::Built {
                        path,
                        result,
                    });
                })
            }
            Event::Built {
                path,
                result,
            } => {
                match result {
                    Ok(()) => info!("Built variant. path: '{}'", path.display()),
                    Err(error) => fail(model, error),
                }
                render::render()
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        OperationViewModel {
            error: model.error.clone(),
            variant_builders: model.variant_builders.clone(),
        }
    }
}

/// Logs why an event failed, and keeps it for the view (`VARB-6`, `LOG-2`).
fn fail(model: &mut Model, error: VariantBuilderError) {
    warn!("Event failed. error: {:?}", error);
    model.error = Some(error.to_string());
}

#[derive(Error, Debug)]
enum AppError {
    #[error("Operation error, cause: {0}")]
    OperationError(anyhow::Error),
}

/// Reads each data source, by the stores service, builds the variant, then writes its placements, and its BOM, by the
/// stores service, as documents whose contents the core makes (`STORE-14`). The first that can't be read or written
/// says why, and stops the rest.
async fn build(ctx: &Context, args: BuildArgs) -> Result<(), String> {
    let inputs = read_inputs(ctx, &args).await?;

    // the variant is built between the reads and the writes, as the rules it's built with can't be held across them.
    let outputs =
        build_assembly_variant(&args, inputs).map_err(|cause| format!("{:?}", AppError::OperationError(cause)))?;

    write_document(ctx, &PathBuf::from(&args.output), outputs.placements).await?;
    info!("Output written to '{}'", args.output);

    if let (Some(output_bom), Some(bom)) = (&args.output_bom, outputs.bom) {
        write_document(ctx, &PathBuf::from(output_bom), bom).await?;
        info!("BOM written to '{}'", output_bom);
    }

    Ok(())
}

/// The records of each data source, in the order they were read when the core read them itself.
async fn read_inputs(ctx: &Context, args: &BuildArgs) -> Result<Inputs, String> {
    let eda_placements = match read_required(
        ctx,
        RecordsKind::EdaPlacements {
            eda_tool: args.eda_tool,
        },
        &args.placements,
    )
    .await?
    {
        RecordsOf::EdaPlacements {
            placements, ..
        } => placements,
        records => return Err(unexpected(&records)),
    };

    let mut substitutions = vec![];
    for source in args.substitutions.iter() {
        match read_required(ctx, RecordsKind::Substitutions, source).await? {
            RecordsOf::Substitutions(rows) => substitutions.push((source.clone(), rows)),
            records => return Err(unexpected(&records)),
        }
    }

    let parts = read_parts(ctx, &args.parts).await?;

    let part_mappings = match read_required(ctx, RecordsKind::PartMappings, &args.part_mappings).await? {
        RecordsOf::PartMappings(rows) => rows,
        records => return Err(unexpected(&records)),
    };

    let load_out_items = match &args.load_out {
        Some(source) => match read_required(ctx, RecordsKind::LoadOut, source).await? {
            RecordsOf::LoadOut(items) => items,
            records => return Err(unexpected(&records)),
        },
        None => vec![],
    };

    let assembly_rules = match &args.assembly_rules {
        Some(source) => match read_required(ctx, RecordsKind::AssemblyRules, source).await? {
            RecordsOf::AssemblyRules(rows) => rows,
            records => return Err(unexpected(&records)),
        },
        None => vec![],
    };

    Ok(Inputs {
        eda_placements,
        substitutions,
        parts,
        part_mappings,
        load_out_items,
        assembly_rules,
    })
}

/// The records of a data source that must exist, read by the stores service (`STORE-11`).
async fn read_required(ctx: &Context, kind: RecordsKind, source: &Source) -> Result<RecordsOf, String> {
    match ctx
        .request_from_shell(StoresOperation::ReadRecords {
            kind,
            source: source.clone(),
        })
        .await
    {
        StoresOutput::RecordsRead(Ok(Some(records))) => Ok(records.records),
        StoresOutput::RecordsRead(Ok(None)) => Err(format!("Data source not found. source: '{}'", source)),
        StoresOutput::RecordsRead(Err(cause)) => Err(cause),
        output => Err(format!("Unexpected output. output: {:?}", output)),
    }
}

/// The parts, read by the stores service.
async fn read_parts(ctx: &Context, source: &PartsSource) -> Result<Vec<PartsViewItem>, String> {
    match ctx
        .request_from_shell(StoresOperation::ReadParts {
            source: source.clone(),
        })
        .await
    {
        StoresOutput::PartsRead(Ok(parts)) => Ok(parts.parts),
        StoresOutput::PartsRead(Err(cause)) => Err(cause),
        output => Err(format!("Unexpected output. output: {:?}", output)),
    }
}

/// Writes the document, by the stores service (`STORE-10`).
async fn write_document(ctx: &Context, path: &PathBuf, contents: String) -> Result<Revision, String> {
    match ctx
        .request_from_shell(StoresOperation::WriteDocument {
            path: path.clone(),
            contents,
        })
        .await
    {
        StoresOutput::DocumentWritten(result) => result,
        output => Err(format!("Unexpected output. output: {:?}", output)),
    }
}

fn unexpected(records: &RecordsOf) -> String {
    format!("Unexpected records. kind: {:?}", records.kind())
}

/// Builds the variant from the data sources' records, logging as it goes, and makes the contents of its files.
#[tracing::instrument(level = Level::DEBUG, skip_all)]
fn build_assembly_variant(args: &BuildArgs, inputs: Inputs) -> Result<Outputs, Error> {
    let BuildArgs {
        eda_tool,
        assembly_variant,
        output_bom,
        ref_des_exclude_list,
        ref_des_disable_list,
        ..
    } = args;
    let eda_tool = *eda_tool;
    let assembly_variant = assembly_variant.clone();

    let original_eda_placements = inputs.eda_placements;
    info!("Loaded {} placements", original_eda_placements.len());

    // FUTURE show unmatched elements from the `ref_des_exclude_list`
    info!("excluding placements: {:?}", ref_des_exclude_list);
    let mut eda_placements: Vec<EdaPlacement> = original_eda_placements
        .into_iter()
        .filter(|placement| !ref_des_exclude_list.contains(&placement.ref_des))
        .collect();

    let eda_substitution_rules = inputs
        .substitutions
        .iter()
        .try_fold(vec![], |mut rules, (source, rows)| {
            let source_rules = substitutions::build_eda_substitutions(rows)?;
            info!("Loaded {} substitution rules from {}", source_rules.len(), source);
            rules.extend(source_rules);

            Ok::<Vec<EdaSubstitutionRule>, anyhow::Error>(rules)
        })?;

    let eda_substitution_results =
        EdaSubstitutor::substitute(eda_placements.as_mut_slice(), eda_substitution_rules.as_slice());
    trace!("eda_substitution_results: {:?}", eda_substitution_results);

    // FUTURE show unmatched elements from the `ref_des_disable_list`
    info!("disabling placements: {:?}", ref_des_disable_list);
    let mut eda_placements: Vec<EdaPlacement> = eda_substitution_results
        .iter()
        .map(|esr| esr.resulting_placement.clone())
        .collect();

    for eda_placement in eda_placements.iter_mut() {
        if ref_des_disable_list.contains(&eda_placement.ref_des) {
            eda_placement.place = false;
        }
    }

    let parts_and_meta_data: Vec<(Part, HashMap<String, String>)> = inputs
        .parts
        .into_iter()
        .map(|item| (item.part, item.fields.into_iter().collect()))
        .collect();
    info!("Loaded {} parts", parts_and_meta_data.len());

    // FUTURE avoid cloning parts
    let parts: Vec<Part> = parts_and_meta_data
        .iter()
        .map(|(part, _meta_data)| part.clone())
        .collect();

    let part_mappings = part_mappings::build_part_mappings(&parts, &inputs.part_mappings)?;
    info!("Loaded {} part mappings", part_mappings.len());
    trace!("{:?}", part_mappings);

    let load_out_items = inputs.load_out_items;
    info!("Loaded {} load-out items", load_out_items.len());

    let assembly_rules = assembly_rules::build(&inputs.assembly_rules)?;
    info!("Loaded {} assembly rules", assembly_rules.len());

    info!("Assembly variant: {}", assembly_variant.name);
    info!("Ref_des list: {}", assembly_variant.ref_des_list.join(", "));

    let result = AssemblyVariantProcessor::process(&eda_placements, assembly_variant)?;
    let variant_placements = result.placements;
    let variant_placements_count = variant_placements.len();

    info!("Matched {} placements for assembly variant", variant_placements_count);

    trace!("{:?}", part_mappings);

    let processing_result = PartMapper::process(&variant_placements, &part_mappings, &load_out_items, &assembly_rules);

    trace!("{:?}", processing_result);

    let matched_mappings = match &processing_result {
        Ok(mappings) => mappings,
        Err(PartMapperError::MappingErrors(mappings)) => mappings,
    };

    let tree = build_mapping_tree(matched_mappings, eda_substitution_results);
    info!("{}", tree);

    match &processing_result {
        Ok(_) => {
            info!("All placements mapped!")
        }
        Err(PartMapperError::MappingErrors(mappings)) => {
            let error_count = mappings
                .iter()
                .filter(|result| result.mapping_result.is_err())
                .count();
            error!("{:?} Mapping failure(s)", error_count)
        }
    }

    let placements = output_csv(matched_mappings)?;

    let bom = match output_bom {
        Some(_) => Some(output_bom_csv(eda_tool, matched_mappings, &parts_and_meta_data)?),
        None => None,
    };

    Ok(Outputs {
        placements,
        bom,
    })
}

/// The variant's placements, as CSV.
fn output_csv(matched_mappings: &Vec<PlacementPartMappingResult>) -> anyhow::Result<String> {
    let mut writer = csv::WriterBuilder::new()
        .quote_style(QuoteStyle::Always)
        .from_writer(vec![]);

    for matched_mapping in matched_mappings.iter() {
        match matched_mapping {
            PlacementPartMappingResult {
                eda_placement,
                part,
                ..
            } => {
                let empty_value = "".to_string();
                let record = PlacementRecord {
                    ref_des: eda_placement.ref_des.clone(),
                    manufacturer: part.map_or_else(|| empty_value.clone(), |part| part.manufacturer.clone()),
                    mpn: part.map_or_else(|| empty_value.clone(), |part| part.mpn.clone()),
                    place: eda_placement.place,
                    pcb_side: (&eda_placement.pcb_side).into(),
                    x: eda_placement.x,
                    y: eda_placement.y,
                    rotation: eda_placement.rotation,
                };

                writer.serialize(record)?;
            }
        }
    }

    csv_contents(writer)
}

/// The writer's CSV.
fn csv_contents(writer: csv::Writer<Vec<u8>>) -> anyhow::Result<String> {
    let contents = writer
        .into_inner()
        .map_err(|error| anyhow::anyhow!("{}", error))?;
    Ok(String::from_utf8(contents)?)
}

/// The variant's BOM, as CSV.
fn output_bom_csv(
    eda_tool: EdaTool,
    matched_mappings: &Vec<PlacementPartMappingResult>,
    parts_and_meta_data: &Vec<(Part, HashMap<String, String>)>,
) -> anyhow::Result<String> {
    // FIXME normal = headers, JLCPCB = no-headers.
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .quote_style(QuoteStyle::Always)
        .from_writer(vec![]);

    let parts_and_meta_data: HashMap<Part, HashMap<String, String>> = parts_and_meta_data
        .iter()
        .cloned()
        .collect();

    let mut parts: BTreeMap<Part, (BTreeSet<RefDes>, String)> = BTreeMap::new();

    trace!("parts: {:?}", parts);

    for matched_mapping in matched_mappings.iter() {
        match matched_mapping {
            PlacementPartMappingResult {
                eda_placement,
                part,
                ..
            } => {
                let footprint = match eda_tool {
                    EdaTool::DipTrace => eda_placement
                        .fields
                        .iter()
                        .find(|it| it.name == "name")
                        .map(|field| field.value.clone()),
                    EdaTool::KiCad => None,
                    EdaTool::EasyEda => None,
                }
                .unwrap_or("UNKNOWN".to_string());

                if let Some(part) = *part {
                    let (ref_des_set, _footprint) = parts
                        .entry(part.clone())
                        .or_insert((BTreeSet::new(), footprint));

                    let ref_des = RefDes::from(eda_placement.ref_des.clone());
                    ref_des_set.insert(ref_des);
                }
            }
        }
    }

    for (part, (ref_des_set, footprint)) in parts.into_iter() {
        let quantity = ref_des_set.len();
        let ref_des_set_joined = ref_des_set
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        // let bom_record = BOMRecord {
        //     ref_des_set: ref_des_set_joined,
        //     manufacturer: part.manufacturer,
        //     mpn: part.mpn,
        //     quantity,
        // };

        let meta_data = parts_and_meta_data.get(&part);

        let bom_record = JLCPCBBOMRecord {
            comment: format!("{} ({})", part.mpn, part.manufacturer),
            designator: ref_des_set_joined,
            footprint,
            jlcpcb_part: meta_data.and_then(|part_meta_data| part_meta_data.get("LCSC").cloned()),
            quantity,
        };

        writer.serialize(bom_record)?;
    }

    csv_contents(writer)
}

fn build_mapping_tree(
    matched_mappings: &Vec<PlacementPartMappingResult>,
    eda_substitution_results: Vec<EdaSubstitutionResult>,
) -> Tree<String> {
    let mut tree = Tree::new("Mapping Result".to_string());

    for PlacementPartMappingResult {
        eda_placement,
        mapping_result: part_mappings_result,
        ..
    } in matched_mappings.iter()
    {
        fn add_error_node(placement_node: &mut Tree<String>, reason: &str) {
            let placement_error_node = Tree::new(format!("ERROR: Unresolved mapping - {}.", reason).to_string());
            placement_node
                .leaves
                .push(placement_error_node);
        }

        if let Some(substitution_result) = eda_substitution_results
            .iter()
            .find(|candidate| {
                candidate
                    .original_placement
                    .ref_des
                    .eq(&eda_placement.ref_des)
            })
        {
            let placement_label = format!(
                "{} ({})",
                eda_placement.ref_des,
                EdaPlacementTreeFormatter::format(
                    &substitution_result
                        .original_placement
                        .fields
                        .as_slice()
                )
            );
            let mut placement_node = Tree::new(placement_label);

            let mut parent = &mut placement_node;

            for chain_entry in substitution_result.chain.iter() {
                let mut substitution_label = format!(
                    "Substituted ({}), by ({})",
                    chain_entry.rule.format_transform(),
                    chain_entry.rule.format_criteria(),
                );

                for classification in &chain_entry.rule.classifications {
                    let (label, message) = match classification {
                        EdaSubstitutionRuleClassification::Derate(message) => ("de-rate", message),
                        EdaSubstitutionRuleClassification::Comment(message) => ("comment", message),
                        EdaSubstitutionRuleClassification::Warning(message) => ("warning", message),
                    };

                    if let Some(message) = message {
                        let append = format!(", {}: '{}'", label, message);
                        substitution_label.push_str(&append);
                    }
                }

                let substitution_node = Tree::new(substitution_label);
                parent.leaves.push(substitution_node);
                parent = parent.leaves.last_mut().unwrap();
            }

            match part_mappings_result {
                Ok(part_mapping_results) => {
                    add_mapping_nodes(part_mapping_results, parent);
                }
                Err(PartMappingError::ConflictingRules(part_mapping_results)) => {
                    add_mapping_nodes(part_mapping_results, parent);
                    add_error_node(parent, "Conflicting rules");
                }
                Err(PartMappingError::NoRulesApplied(part_mapping_results)) => {
                    add_mapping_nodes(part_mapping_results, parent);
                    add_error_node(parent, "No rules applied");
                }
                Err(PartMappingError::NoMappings) => {
                    add_error_node(parent, "No mappings found");
                }
            }

            tree.leaves.push(placement_node)
        };
    }

    tree
}

fn add_mapping_nodes(part_mapping_results: &Vec<PartMappingResult>, placement_node: &mut Tree<String>) {
    for PartMappingResult {
        part_mapping,
        applied_rule,
    } in part_mapping_results.iter()
    {
        let part_chunk = format!(
            "manufacturer: '{}', mpn: '{}'",
            part_mapping.part.manufacturer, part_mapping.part.mpn
        );
        let mut chunks = vec![part_chunk];

        if let Some(rule) = applied_rule {
            let rule_chunk = format!("({})", rule);
            chunks.push(rule_chunk);
        }

        let part_label = chunks.join(" ");

        let part_node = Tree::new(part_label);
        placement_node.leaves.push(part_node);
    }
}

struct EdaPlacementTreeFormatter {}

impl EdaPlacementTreeFormatter {
    fn format(fields: &[EdaPlacementField]) -> String {
        let chunks: Vec<String> = fields
            .iter()
            .map(|field| format!("{}: '{}'", field.name, field.value))
            .collect();
        format!("{}", chunks.join(", "))
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;

    #[test]
    fn minimal() {
        let app = VariantBuilder;
        let mut model = Model::default();

        // Call 'update' and request effects
        app.update(Event::None, &mut model)
            .expect_only_render();

        // Make sure the view matches our expectations
        let actual_view = app.view(&model);
        let expected_view = OperationViewModel::default();
        assert_eq!(actual_view, expected_view);
    }
}
