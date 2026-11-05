# Variant Builder — Requirement Specifications

The Variant Builder turns a design's EDA placements into an assembly variant's placements, and its BOM, using the
user's parts, part mappings, substitutions, assembly rules and load-out. It has one core, `planning/variantbuilder_app`,
used by its CLI, `planning/variantbuilder_cli`, and, as the mission
[`../../missions/variant-builder.md`](../../missions/variant-builder.md) goes on, by the planner's GUI.

The planner's [principles and conventions](../planner/README.md) apply here too: requirement IDs, statuses, acceptance
criteria, logging (`LOG`) and the stores service (`STORE`).

| File | Area | ID prefix |
|---|---|---|
| [core/01-variant-builders.md](core/01-variant-builders.md) | A variant builder's file: creating, opening, configuring and building it | `VARB` |

## Glossary

| Term | Meaning |
|---|---|
| **Variant builder** | A file, `<name>.variant.json`, that says which variant to build, from which data sources, and where its placements and BOM are written. |
| **Variant** | The placements of a design to assemble, named, for example "`Default`", and optionally limited to a list of ref-des. |

## Change log

| Date | Change |
|---|---|
| 2026-11-05 | A variant builder is a file, `<name>.variant.json`, which the core creates, opens, configures and builds from, through the stores service (`VARB-1` to `VARB-6`), so that a variant is defined once, rather than with every build's arguments. |
