# CLI 01 — Commands

The Variant Builder's CLI, `variantbuilder_cli`, creates a variant builder's file, configures it, and builds its
variant, a command at a time, with the same terms as the GUI: create, configure and build. Each command takes the
file with `--file <FILE>`, and sends the core one event (`VARB`).

**VARBCLI-1 — `create`.** *Confirmed.* `variantbuilder_cli create --file <FILE>` MUST create a variant builder
(`VARB-2`). If the file exists, it MUST fail with the core's error, and leave the file as it was, unless `--overwrite`
is given, which replaces it.

**VARBCLI-2 — `configure`.** *Confirmed.* `variantbuilder_cli configure --file <FILE>` MUST configure a variant builder
(`VARB-4`), changing only the options given, which are the arguments `build` took before it built from a file:
`--eda`, `--placements`, `--parts`, `--part-mappings`, `--substitutions`, `--load-out`, `--assembly-rules`,
`--ref-des-exclude-list`, `--ref-des-disable-list`, `--output`, `--output-bom`, `--name` and `--ref-des-list`. A list
given with no values MUST clear it, and `--clear <OPTION>,...` MUST clear each option named that isn't a list. A
relative path is relative to the folder the CLI is run in, as for any command, so the CLI MUST make it relative to the
variant builder's folder (`VARB-1`), or absolute if it's outside it. If the core refuses, the CLI MUST fail with its
error.

**VARBCLI-3 — `build`.** *Confirmed.* `variantbuilder_cli build --file <FILE>` MUST build the variant builder's
variant (`VARB-5`), and if the core refuses, fail with its error.

## Acceptance criteria

- `planning/variantbuilder_cli/tests/variantbuilder.rs` creates, configures and builds variants of DipTrace, KiCad and
  EasyEDA designs, with the arguments `build` took before, and checks the placements are as before, and that
  "`variantbuilder_app: Received event. event: Create {`", "`variantbuilder_app: Created variant builder. path: '`",
  "`variantbuilder_app: Received event. event: Configure {`",
  "`variantbuilder_app: Configured variant builder. path: '`", "`variantbuilder_app: Received event. event: Build {`",
  "`variantbuilder_app::variant_builders: Building variant. path: '`", the "`Loaded`" lines, and
  "`variantbuilder_app: Built variant. path: '`" are logged, in order, with no "`Event failed.`" (`VARB-6`,
  `STORE-14`). It checks the help for the commands and for `configure`.
- `planning/variantbuilder_cli/tests/variant_builders.rs` checks `create` refuses to replace a file without
  `--overwrite`, logging "`variantbuilder_app: Event failed. error: ExistsAlready {`", and replaces it with it
  (`VARBCLI-1`), that `configure` changes only the options given, with paths relative to the variant builder's folder,
  or absolute outside it, and clears a list given with no values and the options named by `--clear`, and that a
  variant builder that doesn't exist isn't configured (`VARBCLI-2`), and that `build` names what an incomplete variant
  builder is missing, and reads no data source (`VARBCLI-3`).
