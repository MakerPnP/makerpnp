use std::collections::BTreeMap;
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::{json, Map, Value};

#[derive(Default)]
pub struct TestProjectBuilder<'a> {
    name: Option<&'a str>,
    processes: Option<&'a [&'a str]>,
    pcbs: Option<&'a [(&'a str, &'a str)]>,
    unit_assignments: Option<&'a[(&'a str, BTreeMap<&'a str, &'a str>)]>,
    part_states: Option<&'a [((&'a str, &'a str), &'a [&'a str])]>,
    placements: Option<&'a [
        (&'a str, &'a str, (
            &'a str, &'a str, &'a str, bool, &'a str, Decimal, Decimal, Decimal
        ), bool, &'a str, Option<&'a str>)
    ]>,
    phases: Option<&'a [(&'a str, &'a str, &'a str, &'a str, &'a [(&'a str, &'a str)])]>,
}

impl<'a> TestProjectBuilder<'a> {
    pub fn content(&self) -> String {
        let mut root = json!({});

        if let Some(name) = self.name {
            root["name"] = Value::String(name.to_string());
        }

        if let Some(processes) = self.processes {
            root["processes"] = Value::Array(processes
                .to_vec().iter()
                .map(|process|Value::String(process.to_string())).collect()
            );
        }

        if let Some(pcbs) = self.pcbs {
            root["pcbs"] = Value::Array(pcbs
                .to_vec().iter()
                .map(|(kind, name)| {
                    let mut pcb_map = Map::new();
                    pcb_map.insert("kind".to_string(), Value::String(kind.to_string()));
                    pcb_map.insert("name".to_string(), Value::String(name.to_string()));
                    Value::Object(pcb_map)
                }).collect()
            );
        }

        if let Some(unit_assignments) = self.unit_assignments {

            let values: Vec<Value> = unit_assignments.iter().map(|(key, values)|{
                Value::Array(vec![
                    Value::String(key.to_string()),
                    Value::Object(values.iter().fold(Map::new(), | mut map, (k,v)| {
                        map.insert(k.to_string(), Value::String(v.to_string()));

                        map
                    })),
                ])
            }).collect();

            root["unit_assignments"] = Value::Array(values);
        }

        if let Some(part_states) = self.part_states {
            let values: Vec<Value> = part_states.iter().map(|((manufacturer, mpn), applicable_processes)|{

                let mut part_map = Map::new();
                part_map.insert("manufacturer".to_string(), Value::String(manufacturer.to_string()));
                part_map.insert("mpn".to_string(), Value::String(mpn.to_string()));


                let mut state_map = Map::new();
                if !applicable_processes.is_empty() {
                    state_map.insert("applicable_processes".to_string(), Value::Array(applicable_processes
                        .to_vec().iter()
                        .map(|process| Value::String(process.to_string())).collect()
                    ));
                }

                Value::Array(vec![
                    Value::Object(part_map),
                    Value::Object(state_map)
                ])
            }).collect();

            root["part_states"] = Value::Array(values);
        }

        if let Some(phases) = self.phases {
            let values: Vec<Value> = phases.iter().map(|(reference, process, load_out_source, pcb_side, sort_orderings)| {
                let mut phase_map = Map::new();
                phase_map.insert("reference".to_string(), Value::String(reference.to_string()));
                phase_map.insert("process".to_string(), Value::String(process.to_string()));
                phase_map.insert("load_out".to_string(), Value::String(load_out_source.to_string()));
                phase_map.insert("pcb_side".to_string(), Value::String(pcb_side.to_string()));

                if !sort_orderings.is_empty() {
                    let sort_orderings_values: Vec<Value> = sort_orderings.iter().map(|(mode, sort_order)| {
                        let mut ordering_map= Map::new();
                        ordering_map.insert("mode".to_string(), Value::String(mode.to_string()));
                        ordering_map.insert("sort_order".to_string(), Value::String(sort_order.to_string()));
                        Value::Object(ordering_map)
                    }).collect();
                    phase_map.insert("sort_orderings".to_string(), Value::Array(sort_orderings_values));
                }

                Value::Array(vec![
                    Value::String(reference.to_string()),
                    Value::Object(phase_map),
                ])
            }).collect();
            root["phases"] = Value::Array(values);
        }

        if let Some(placements) = self.placements {

            let values: Vec<Value> = placements.iter().map(|(
                                                                key,
                                                                unit_path, (
                    ref_des, manufacturer, mpn, place, pcb_side, x, y , rotation
                ),
                                                                placed,
                                                                status,
                                                                phase,
                                                            ) | {

                let mut part_map = Map::new();
                part_map.insert("manufacturer".to_string(), Value::String(manufacturer.to_string()));
                part_map.insert("mpn".to_string(), Value::String(mpn.to_string()));

                let mut placement_map = Map::new();
                placement_map.insert("ref_des".to_string(), Value::String(ref_des.to_string()));
                placement_map.insert("part".to_string(), Value::Object(part_map));
                placement_map.insert("place".to_string(), Value::Bool(*place));
                placement_map.insert("pcb_side".to_string(), Value::String(pcb_side.to_string()));
                placement_map.insert("x".to_string(), Value::String(x.to_string()));
                placement_map.insert("y".to_string(), Value::String(y.to_string()));
                placement_map.insert("rotation".to_string(), Value::String(rotation.to_string()));

                let mut placement_state_map = Map::new();
                placement_state_map.insert("unit_path".to_string(), Value::String(unit_path.to_string()));
                placement_state_map.insert("placement".to_string(), Value::Object(placement_map));
                placement_state_map.insert("placed".to_string(), Value::Bool(*placed));
                placement_state_map.insert("status".to_string(), Value::String(status.to_string()));

                if let Some(phase) = phase {
                    placement_state_map.insert("phase".to_string(), Value::String(phase.to_string()));
                }

                Value::Array(vec![
                    Value::String(key.to_string()),
                    Value::Object(placement_state_map),
                ])
            }).collect();

            root["placements"] = Value::Array(values);
        }

        let mut buffer = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut ser = serde_json::Serializer::with_formatter(&mut buffer, formatter);

        root.serialize(&mut ser).expect("TODO");

        let mut content = String::from_utf8(buffer).unwrap();
        content.push('\n');

        content
    }

    pub fn with_pcbs(mut self, pcbs: &'a [(&'a str, &'a str)]) -> Self {
        self.pcbs = Some(pcbs);
        self
    }

    pub fn with_phases(mut self, phases: &'a [(&'a str, &'a str, &'a str, &'a str, &'a [(&'a str, &'a str)])]) -> Self {
        self.phases = Some(phases);
        self
    }

    pub fn with_placements(mut self, placements: &'a [
        (&'a str, &'a str, (
            &'a str, &'a str, &'a str, bool, &'a str, Decimal, Decimal, Decimal,
        ), bool, &'a str, Option<&'a str>)
    ]) -> Self {
        self.placements = Some(placements);
        self
    }

    pub fn with_part_states(mut self, part_states: &'a [((&'a str, &'a str), &'a [&'a str])]) -> Self {
        self.part_states = Some(part_states);
        self
    }

    pub fn with_unit_assignments(mut self, unit_assignments: &'a [(&'a str, BTreeMap<&'a str, &'a str>)]) -> Self {
        self.unit_assignments = Some(unit_assignments);
        self
    }
    pub fn with_processes(mut self, processes: &'a [&'a str]) -> Self {
        self.processes = Some(processes);
        self
    }

    pub fn with_name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    pub fn new() -> Self {
        Default::default()
    }
}
