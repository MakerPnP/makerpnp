# CLI 01 — Commands

The Variant Builder's CLI, `variantbuilder_cli`, creates a variant builder's file, configures it, and builds its
variant, a command at a time, with the same terms as the GUI: create, configure and build, and creates assembly rules
for it, as the GUI's "`Assembly rules: New`" does. Each command takes the file with `--file <FILE>`, and sends the core
one event (`VARB`).

**VARBCLI-1 — `create`.** *Confirmed.* `variantbuilder_cli create --file <FILE>` MUST create a variant builder
(`VARB-2`), with `--design-name` and `--variant-name`, if they're given, which only `create` sets, and the options
`configure` takes (`VARBCLI-2`), if they're given. If the file exists, it MUST fail with the core's error, and leave the
file as it was, unless `--overwrite` is given, which replaces it.

**VARBCLI-2 — `configure`.** *Confirmed.* `variantbuilder_cli configure --file <FILE>` MUST configure a variant builder
(`VARB-4`), changing only the options given: `--eda`, `--placements`, `--parts`, `--part-mappings`,
`--global-substitutions`, `--design-substitutions`, `--load-out`, `--assembly-rules`, `--ref-des-list`,
`--ref-des-exclusion-list`, `--ref-des-no-place-list`, `--output` and `--output-bom`. A list given with no values MUST
clear it, and `--clear <OPTION>,...` MUST clear each option named that isn't a list. A relative path is relative to the
folder the CLI is run in, as for any command, so the CLI MUST make it relative to the variant builder's folder
(`VARB-1`), or absolute if it's outside it. If the core refuses, the CLI MUST fail with its error.

**VARBCLI-3 — `build`.** *Confirmed.* `variantbuilder_cli build --file <FILE>` MUST build the variant builder's
variant (`VARB-5`), and if the core refuses, fail with its error.

**VARBCLI-4 — `create-assembly-rules`.** *Confirmed.*
`variantbuilder_cli create-assembly-rules --file <FILE> --assembly-rules <FILE>` MUST create assembly rules for the
variant builder (`VARB-8`), with the path made relative as `configure` makes them, and if the core refuses, fail with
its error.

## Acceptance criteria

- `planning/variantbuilder_cli/tests/variantbuilder.rs` creates, configures and builds variants of DipTrace, KiCad and
  EasyEDA designs, with the arguments `build` took before it built from a file, renamed, and checks the placements are
  as before, and that
  "`variantbuilder_app: Received event. event: Create {`", "`variantbuilder_app: Created variant builder. path: '`",
  "`variantbuilder_app: Received event. event: Configure {`",
  "`variantbuilder_app: Configured variant builder. path: '`", "`variantbuilder_app: Received event. event: Build {`",
  "`variantbuilder_app::variant_builders: Building variant. path: '`", the "`Loaded`" lines, and
  "`variantbuilder_app: Built variant. path: '`" are logged, in order, with no "`Event failed.`" (`VARB-6`,
  `STORE-14`). It checks the help for the commands and for `configure`.
- `planning/variantbuilder_cli/tests/variant_builders.rs` checks `create` refuses to replace a file without
  `--overwrite`, logging "`variantbuilder_app: Event failed. error: ExistsAlready {`", and replaces it with it
  (`VARBCLI-1`), that `create` sets the options given, and names the design after its placements' file
  (`VARBCLI-1`), that `configure` changes only the options given, with paths relative to the variant builder's folder,
  or absolute outside it, and clears a list given with no values and the options named by `--clear`, and that a
  variant builder that doesn't exist isn't configured (`VARBCLI-2`), that `build` names what an incomplete variant
  builder is missing, and reads no data source (`VARBCLI-3`), and that `create-assembly-rules` writes a file with no
  rules, sets the variant builder's to it, logging
  "`variantbuilder_app: Created assembly rules. path: 'job1.variant.json', assembly_rules: 'rules.csv'`", and refuses to
  replace a file (`VARBCLI-4`).
