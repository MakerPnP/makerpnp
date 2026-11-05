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
| [cli/01-commands.md](cli/01-commands.md) | The CLI's `create`, `configure`, `build` and `create-assembly-rules` commands | `VARBCLI` |

The planner's GUI's design variant tabs are specified with the planner's GUI, in
[`../planner/gui/13-variant-builders.md`](../planner/gui/13-variant-builders.md) (`VBGUI`).

## Glossary

| Term | Meaning |
|---|---|
| **Variant builder** | A file, `<design name>_<variant name>.variant.json`, that says which variant to build, from which data sources, and where its placements and BOM are written. |
| **Design variant** | What the GUI calls a variant builder, as the user sees it: a variant of a design. |
| **Variant** | The placements of a design to assemble, named, for example "`default`", and optionally limited to a list of ref-des. |
| **Global substitutions** | Substitutions shared by designs, applied after the design's own, its **design substitutions**. |

## Change log

| Date | Change |
|---|---|
| 2026-11-05 | A variant builder is a file, `<name>.variant.json`, which the core creates, opens, configures and builds from, through the stores service (`VARB-1` to `VARB-6`), so that a variant is defined once, rather than with every build's arguments. |
| 2026-11-05 | The CLI creates, configures and builds a variant builder's file: `variantbuilder_cli create --file <file>`, refusing to replace one without `--overwrite`, `configure`, with the arguments `build` took before, each optional, and `build --file <file>`, which no longer takes them (`VARBCLI-1` to `VARBCLI-3`). The core no longer builds without a file, and an unset BOM is written to `<name>_bom.csv` (`VARB-5`). |
| 2026-11-05 | The core's view says, for each variant builder, whether an event for it is being processed, why its last one failed, and whether its variant was built (`VARB-7`), which the planner's GUI shows (`VBGUI`). A path is kept relative to the variant builder's folder by the core's `stored_path`, which the CLI and the GUI share. |
| 2026-11-05 | A variant builder's file has a design name and a variant name, set only as it's created, the design's named after its placements unless it's named, and its fields are renamed and grouped: `substitutions` is split into `global_substitutions` and `design_substitutions`, the design's applied first, `variant` into `variant_name` and `ref_des_list`, and `ref_des_exclude_list` and `ref_des_disable_list` are renamed `ref_des_exclusion_list` and `ref_des_no_place_list` (`VARB-1`, `VARB-2`, `VARB-5`). A new variant's name is "`default`". The CLI's options are renamed to match, `create` takes `configure`'s options, `--design-name` and `--variant-name`, and `configure` no longer takes `--name` (`VARBCLI-1`, `VARBCLI-2`). Assembly rules are created, with none, and the variant builder's set to them (`VARB-8`, `VARBCLI-4`). A file written before isn't read the same: its renamed fields are read as new. The core gives the order the substitutions are applied in, which the GUI shows (`VARB-5`). A list of load-outs, one for each machine, is a target (`VARB-1`). |
