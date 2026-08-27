use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use anyhow::Error;
pub use assembly::assembly_variant::AssemblyVariant;
use assembly::AssemblyVariantProcessor;
pub use crux_core::Core;
use crux_core::{render, App, Command};
use crux_core::macros::effect;
use crux_core::render::RenderOperation;
use csv::QuoteStyle;
use eda::placement::{EdaPlacement, EdaPlacementField};
use eda::substitution::{
    EdaSubstitutionResult, EdaSubstitutionRule, EdaSubstitutionRuleClassification, EdaSubstitutor,
};
pub use eda::EdaTool;
use part_mapper::{PartMapper, PartMapperError, PartMappingError, PartMappingResult, PlacementPartMappingResult};
pub use pnp::part::Part;
pub use pnp::placement::RefDes;
use serde_with::serde_as;
pub use stores::assembly_rules::AssemblyRuleSource;
use stores::bom::{BOMRecord, JLCPCBBOMRecord};
pub use stores::eda_placements::EdaPlacementsSource;
pub use stores::load_out::LoadOutSource;
pub use stores::part_mappings::PartMappingsSource;
pub use stores::parts::PartsSource;
use stores::placements::PlacementRecord;
pub use stores::placements::PlacementsSource;
pub use stores::substitutions::EdaSubstitutionsSource;
use stores::{assembly_rules, eda_placements, load_out, part_mappings, parts, substitutions};
use termtree::Tree;
use thiserror::Error;
use tracing::Level;
use tracing::{error, info, trace};

extern crate serde_regex;

#[derive(Default)]
pub struct VariantBuilder;

#[derive(Default)]
pub struct Model {
    error: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct OperationViewModel {
    pub error: Option<String>,
}

#[effect]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum Event {
    None,
    Build {
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
    },
    //
    // Views
    //
}

impl App for VariantBuilder {
    type Event = Event;
    type Model = Model;
    type ViewModel = OperationViewModel;
    type Effect = Effect;

    fn update(
        &self,
        event: Self::Event,
        model: &mut Self::Model,
    ) -> Command<Self::Effect, Self::Event> {
        match event {
            Event::None => render::render(),
            Event::Build {
                eda_tool,
                placements,
                assembly_variant,
                parts,
                part_mappings,
                substitutions,
                load_out,
                assembly_rules,
                output,
                output_bom,
                ref_des_exclude_list,
                ref_des_disable_list,
            } => {
                let try_fn = |_model: &mut Model| -> Result<Command<Self::Effect, Self::Event>, AppError> {
                    build_assembly_variant(
                        eda_tool,
                        &placements,
                        assembly_variant,
                        &parts,
                        &part_mappings,
                        &substitutions,
                        &load_out,
                        &assembly_rules,
                        &output,
                        &output_bom,
                        &ref_des_exclude_list,
                        &ref_des_disable_list,
                    )
                    .map_err(|cause| AppError::OperationError(cause.into()))?;

                    Ok(render::render())
                };

                match try_fn(model) {
                    Ok(command) => command,
                    Err(e) => {
                        model.error.replace(format!("{:?}", e));
                        render::render()
                    }
                }
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        OperationViewModel {
            error: model.error.clone(),
        }
    }
}

#[derive(Error, Debug)]
enum AppError {
    #[error("Operation error, cause: {0}")]
    OperationError(anyhow::Error),
}

#[tracing::instrument(level = Level::DEBUG)]
fn build_assembly_variant(
    eda_tool: EdaTool,
    placements_source: &EdaPlacementsSource,
    assembly_variant: AssemblyVariant,
    parts_source: &PartsSource,
    part_mappings_source: &PartMappingsSource,
    eda_substitutions_sources: &[EdaSubstitutionsSource],
    load_out_source: &Option<LoadOutSource>,
    assembly_rules_source: &Option<AssemblyRuleSource>,
    output: &String,
    output_bom: &Option<String>,
    ref_des_exclude_list: &Vec<String>,
    ref_des_disable_list: &Vec<String>,
) -> Result<(), Error> {
    let original_eda_placements = eda_placements::load_eda_placements(eda_tool, placements_source)?;
    info!("Loaded {} placements", original_eda_placements.len());

    // FUTURE show unmatched elements from the `ref_des_exclude_list`
    info!("excluding placements: {:?}", ref_des_exclude_list);
    let mut eda_placements: Vec<EdaPlacement> = original_eda_placements
        .into_iter()
        .filter(|placement| !ref_des_exclude_list.contains(&placement.ref_des))
        .collect();

    let eda_substitution_rules = eda_substitutions_sources
        .iter()
        .try_fold(vec![], |mut rules, source| {
            let source_rules = substitutions::load_eda_substitutions(source)?;
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

    let parts_and_meta_data = parts::load_parts(parts_source)?;
    info!("Loaded {} parts", parts_and_meta_data.len());

    // FUTURE avoid cloning parts
    let parts = parts_and_meta_data
        .iter()
        .map(|(part, _meta_data)| part.clone())
        .collect();

    let part_mappings = part_mappings::load_part_mappings(&parts, part_mappings_source)?;
    info!("Loaded {} part mappings", part_mappings.len());
    trace!("{:?}", part_mappings);

    let load_out_items = match load_out_source {
        Some(source) => load_out::load_items(source),
        None => Ok(vec![]),
    }?;
    info!("Loaded {} load-out items", load_out_items.len());

    let assembly_rules = match assembly_rules_source {
        Some(source) => assembly_rules::load(source),
        None => Ok(vec![]),
    }?;
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

    write_output_csv(output, matched_mappings)?;

    info!("Output written to '{}'", output);

    if let Some(output_bom) = output_bom {
        write_output_bom_csv(eda_tool, output_bom, matched_mappings, &parts_and_meta_data)?;
        info!("BOM written to '{}'", output_bom);
    }

    Ok(())
}

fn write_output_csv(
    output_file_name: &String,
    matched_mappings: &Vec<PlacementPartMappingResult>,
) -> anyhow::Result<()> {
    let output_path = PathBuf::from(output_file_name);

    let mut writer = csv::WriterBuilder::new()
        .quote_style(QuoteStyle::Always)
        .from_path(output_path)?;

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

    writer.flush()?;

    Ok(())
}

fn write_output_bom_csv(
    eda_tool: EdaTool,
    output_file_name: &String,
    matched_mappings: &Vec<PlacementPartMappingResult>,
    parts_and_meta_data: &Vec<(Part, HashMap<String, String>)>,
) -> anyhow::Result<()> {
    let output_path = PathBuf::from(output_file_name);

    // FIXME normal = headers, JLCPCB = no-headers.
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .quote_style(QuoteStyle::Always)
        .from_path(output_path)?;

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

    writer.flush()?;

    Ok(())
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
