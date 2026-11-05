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
| [cli/01-commands.md](cli/01-commands.md) | The CLI's `create`, `configure` and `build` commands | `VARBCLI` |

## Glossary

| Term | Meaning |
|---|---|
| **Variant builder** | A file, `<name>.variant.json`, that says which variant to build, from which data sources, and where its placements and BOM are written. |
| **Variant** | The placements of a design to assemble, named, for example "`Default`", and optionally limited to a list of ref-des. |

## Change log

| Date | Change |
|---|---|
| 2026-11-05 | A variant builder is a file, `<name>.variant.json`, which the core creates, opens, configures and builds from, through the stores service (`VARB-1` to `VARB-6`), so that a variant is defined once, rather than with every build's arguments. |
| 2026-11-05 | The CLI creates, configures and builds a variant builder's file: `variantbuilder_cli create --file <file>`, refusing to replace one without `--overwrite`, `configure`, with the arguments `build` took before, each optional, and `build --file <file>`, which no longer takes them (`VARBCLI-1` to `VARBCLI-3`). The core no longer builds without a file, and an unset BOM is written to `<name>_bom.csv` (`VARB-5`). |
