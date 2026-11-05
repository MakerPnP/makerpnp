# Core 01 — Variant builders

A variant builder is a file that defines a variant, and the data sources it's built from, so that the user defines it
once, changes it a little at a time, and builds it again, from the CLI or the GUI, rather than giving every argument to
every build. The core reads and writes it through the stores service, as a document (`STORE-10`), and does no IO itself
(`STORE-7`).

**VARB-1 — A variant builder's file.** *Confirmed.* A variant builder MUST be a JSON file, named
`<name>.variant.json`, with:

| Field | What | When it's new |
|---|---|---|
| `variant` | The variant's `name`, and its `ref_des_list`: the ref-des it's limited to, or all if it's empty. | "`Default`", all |
| `eda_tool` | The EDA tool whose placements are read: `DipTrace`, `KiCad` or `EasyEda`. | None |
| `placements` | The EDA placements file. | None |
| `parts` | The parts data source. | None |
| `part_mappings` | The part mappings data source. | None |
| `substitutions` | The substitutions data sources, applied in order. | None |
| `load_out` | The load-out data source. | None |
| `assembly_rules` | The assembly rules data source. | None |
| `ref_des_exclude_list` | The ref-des left out of the variant, such as test pads and fiducials. | None |
| `ref_des_disable_list` | The ref-des in the variant that aren't placed, such as do-not-fit parts. | None |
| `output_placements` | The variant's placements file. | None, see `VARB-5` |
| `output_bom` | The variant's BOM file. | None, see `VARB-5` |

A field that's left out of the file MUST be read as it is when it's new, so the file can be written by hand. A data
source or output is a path, and a relative path MUST be relative to the variant builder's folder, so the folder can be
moved with its files.

**VARB-2 — Creating a variant builder.** *Confirmed.* Creating a variant builder (`Create`) MUST write a new one's file,
by the stores service (`STORE-10`), and the variant builder MUST then be open (`VARB-3`). Unless asked to overwrite it,
the core MUST first have the stores service read the file, and if there is one, MUST refuse with
"`Variant builder exists already. path: '<path>'`", and write nothing. Once the file is written, the core MUST log
"`Created variant builder. path: '<path>'`". A file that can't be written MUST say why, and the variant builder isn't
created. *Target*: the file is read, then written, so a file made between the two is overwritten, as the stores service
has no way yet to write a document only if there isn't one.

**VARB-3 — Opening a variant builder.** *Confirmed.* Opening a variant builder (`Open`) MUST have the stores service
read its file, and the core's view MUST then have it, by its path, with what its file says, and the file's revision
(`STORE-8`). Once it's read, the core MUST log "`Opened variant builder. path: '<path>', revision: '<revision>'`". A
file that doesn't exist MUST say "`Variant builder not found. path: '<path>'`", and one that isn't a variant builder
MUST say "`Invalid variant builder. path: '<path>', cause: '<cause>'`", and neither is opened.

**VARB-4 — Configuring a variant builder.** *Confirmed.* Configuring a variant builder (`Configure`) MUST make the
changes asked for to its file, each setting one field (`VARB-1`), or clearing it, and keep the fields not named. The
core MUST have the stores service read the file, make the changes, and have the service write it, so a change made by
anyone since it was opened is kept. Once it's written, the core MUST log
"`Configured variant builder. path: '<path>', change_count: <n>`", and if the variant builder is open, its view MUST be
updated. A file that doesn't exist, or isn't a variant builder, MUST say so as `VARB-3` does, and nothing is written.
*Target*: the file is read, then written, so a change made by anyone between the two is lost.

**VARB-5 — Building a variant.** *Confirmed.* Building a variant builder's variant (`Build`) MUST have the stores
service read its file, then build the variant from it (`STORE-14`), with each relative path relative to the file's
folder (`VARB-1`). The EDA tool, the placements, the parts and the part mappings MUST be set, and if any isn't, the
core MUST refuse with "`Variant builder incomplete. path: '<path>', missing: [<fields>]`", naming each field that isn't
set, before reading any data source. An output that isn't set MUST be the variant builder's name, with
`_placements.csv` or `_bom.csv`, in its folder, so `job1.variant.json` writes `job1_placements.csv` and `job1_bom.csv`.
The core MUST log "`Building variant. path: '<path>'`" once the file is read, then build as `STORE-14` says, then
"`Built variant. path: '<path>'`" once its files are written.

**VARB-6 — The core's logs.** *Confirmed.* The core MUST log each event it receives, at debug level, as
"`Received event. event: <event>`" (`LOG-1`), and why it refused one, as "`Event failed. error: <error>`" (`LOG-2`).

## Acceptance criteria

- `planning/variantbuilder_app/tests/variant_builders.rs`, with no files, answers the operations the core asks for, and
  checks:
  - creating a variant builder reads the file, finds none, writes a new one's, as `VARB-1` says, and opens it, and that
    one with a file already isn't created, and nothing is written, unless asked to overwrite it (`VARB-2`);
  - opening a variant builder reads its file, and the view has what it says and its revision, and that one that doesn't
    exist, or isn't a variant builder, isn't opened, and says why (`VARB-3`);
  - a file with fields left out is read as a new one's (`VARB-1`);
  - configuring a variant builder reads its file, writes it with the changes, keeping the rest, and updates the view
    of one that's open (`VARB-4`);
  - building reads the file, then each data source relative to the file's folder, and writes the placements and BOM
    files named after the variant builder, unless they're set (`VARB-5`, `STORE-14`);
  - building one that isn't complete names each field missing, and reads no data source (`VARB-5`).
- The CLI's tests check the core logs "`variantbuilder_app: Received event. event: Create {`",
  "`variantbuilder_app: Created variant builder. path: '`", "`variantbuilder_app: Received event. event: Build {`",
  "`variantbuilder_app::variant_builders: Building variant. path: '`" and "`variantbuilder_app: Built variant. path: '`", in order
  (`VARB-6`), see `VARBCLI`.
