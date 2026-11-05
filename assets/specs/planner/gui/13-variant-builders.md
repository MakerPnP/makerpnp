# GUI 13 — Design variants

A design variant's tab, which shows a variant builder, in which the user chooses the data sources a variant of a design
is built from, saves them to its file, and builds it, through the Variant Builder's core
(`../../variantbuilder/core/01-variant-builders.md`), as the CLI's `create`, `configure` and `build` do (`VARBCLI`).
It all starts with the design's placements, which the EDA tool wrote. The variant builder's explorer, and the tabs for
its data sources and results, are to come (`../../../missions/variant-builder.md`, step 10).

**VBGUI-1 — New design variant.** *Confirmed.* "`New design variant`" in the toolbar MUST open a
"`New design variant`" tab, with, in order:

- "`EDA tool`", chosen from a list;
- "`Placements`", chosen with a dialog that shows the EDA tool's placements files (`VARB-1`), and only once there's an
  EDA tool, and chosen again if the EDA tool is changed;
- "`Design name`", empty until the placements are chosen, then their file's name, without its extension, unless the
  user has named it;
- "`Variant name`", "`default`" until the user names it.

Each is required before "`Ok`" is enabled. "`Ok`" MUST create `<design name>_<variant name>.variant.json` in the
placements' folder (`VARB-2`), with the EDA tool, the placements, and the names, without replacing a file that's there,
and the tab MUST then be the variant builder's, named after its file. If the core refuses, the tab MUST show why. The
variant builder's tab MUST show its "`EDA tool`", "`Placements`", "`Design name`" and "`Variant name`", and the user
can't change them.

**VBGUI-2 — Open design variant.** *Confirmed.* "`Open design variant`" in the toolbar MUST have the user choose a
variant builder's file with a dialog, and open it in a tab of its own (`VARB-3`), named after its file, which shows
what the file says: first what's set as it's created (`VBGUI-1`), then the rest in groups:

| Group | Fields |
|---|---|
| "`Part matching`" | "`Parts`", "`Part mappings`", "`Global substitutions`" |
| "`Design specific`" | "`Assembly rules`", "`Design substitutions`" |
| "`Reference designators`" | "`Ref-des list`", "`Exclusion list`", "`No-place list`" |
| "`Output`" | "`Output placements`", "`Output BOM`" |

Each ref-des list is shown, and typed, separated by commas. Each list of files is a table, with the columns
"`Order`", the file's place in the list, "`File`", its path, relative to the variant builder's folder or absolute
(`VARB-1`), and "`Actions`", with "`<field>: Move up <file>`" and "`<field>: Move down <file>`" to move it one place up
or down the list, disabled for the first and the last, and "`<field>: Remove <file>`" to remove it, each lined up with
the same button of the other rows, and "`<field>: Add`", below it, adds the files chosen with a dialog. The load-out
isn't shown yet. *Target*: a list of load-outs, one for each pick-and-place machine of the job (`VARB-1`), shown as a
list of files.

**VBGUI-3 — Changing a design variant.** *Confirmed.* The user MUST be able to change each field in the groups: the
text fields by typing, each data source by choosing its file with "`<field>: Choose`", or clearing it with
"`<field>: Clear`", and each list of files with "`<field>: Add`", "`<field>: Move up <file>`",
"`<field>: Move down <file>`" and "`<field>: Remove <file>`" (`VBGUI-2`). A file
chosen MUST be kept relative to the variant builder's folder, or absolute if it's outside it (`VARB-1`). The changes
MUST be shown at once, and kept by the tab until "`Save changes`", enabled once a field differs from what the file
says, configures the variant builder with each field that differs (`VARB-4`). The tab MUST keep the changes if the core
refuses them, so the user doesn't lose them (`FORM-5`). The toolbar's "`Save`" doesn't save a variant builder.

**VBGUI-4 — Building.** *Confirmed.* "`Build`" MUST build the variant builder's variant (`VARB-5`), and so is enabled
only while there are no changes to save, as the variant is built from what the file says, and while nothing else is
being done to the variant builder (`VARB-7`). The tab MUST show a spinner while the core is busy with the variant
builder, "`Built.`" once its variant is built, until it's changed, and why the core refused its last event, if it did.

**VBGUI-5 — Restoring.** *Observed.* A variant builder's tab MUST be shown again when the GUI starts again, and a tab
with a file MUST open it again (`VARB-3`).

**VBGUI-6 — New assembly rules.** *Confirmed.* "`Assembly rules: New`" MUST have the user choose a file to create with
a dialog, which starts in the variant builder's folder, suggesting `<design name>_<variant name>_assembly_rules.csv`,
then create assembly rules there (`VARB-8`), which the variant builder's assembly rules are then set to, replacing a
change to them not yet saved. If the core refuses, the tab MUST show why.

**VBGUI-7 — The substitutions, as they're applied.** *Confirmed.* Below the groups' separator, the tab MUST show
"`Substitutions, in the order they're applied`", and a table of the substitutions, in the order the core applies them
(`VARB-5`): the design's, then the global ones, with the changes not yet saved, so that adding, moving or removing one
shows at once where it's applied. Its columns MUST be "`Order`", its place, "`File`", its path, as in its list, and
"`Source`", "`Design`" or "`Global`". With none, it MUST show "`(none)`" rather than the table.

**VBGUI-8 — The layout.** *Confirmed.* The "`New design variant`" form, and each group of a design variant's fields,
MUST be laid out in two columns: the labels, as wide as the widest label of the form, or of all the groups, and the
fields, filling the rest of the width, so every field is as wide as the others, and lines up with them, and is resized
with the tab. A file's buttons are on the right of its field. The layout doesn't use `egui_taffy`, which is to be
removed from the GUI, but the form helpers' `FieldGrid`, a grid of egui's own.

## Acceptance criteria

- `tests/ui/variant_builders.rs` creates a design variant with "`New design variant`", choosing DipTrace and its
  placements, checks the file is named after the placements, and that what it's of is shown and the placements can't
  be chosen again, chooses its data sources, which are shown relative to its folder, checks "`Build`" is disabled until
  "`Save changes`" is clicked, and that the file is saved, then builds it, and checks "`Built.`" is shown and the
  placements and BOM are written. It checks "`planner_gui_egui::toolbar: Toolbar command. command: NewDesignVariantClicked`",
  the tab's "`EdaToolChanged(Some(DipTrace))`" and "`PickPlacementsClicked`",
  "`planner_gui_egui::file_picker: Picking a file. extensions: [\"csv\"]`", the tab's "`PlacementsPicked(`" and
  "`Submit`", "`variantbuilder_app: Received event. event: Create {`", the stores service's "`ReadDocument`" and its
  reply as a "`VariantBuilderReply`" command, "`WriteDocument`",
  "`variantbuilder_app: Created variant builder. path: '`", the tab's "`PickClicked(Parts)`", "`FilePicked(Parts, `"
  and "`SaveClicked`", "`variantbuilder_app: Received event. event: Configure {`",
  "`variantbuilder_app: Configured variant builder.`", the tab's "`BuildClicked`",
  "`variantbuilder_app: Received event. event: Build {`", "`variantbuilder_app::variant_builders: Building variant.`"
  and "`variantbuilder_app: Built variant.`" are logged, in order, with no "`Event failed.`" (`VBGUI-1`, `VBGUI-3`,
  `VBGUI-4`).
- `tests/ui/variant_builders.rs` checks the fields of the "`New design variant`" form are as wide as each other and
  fill the width, that a design variant's "`Placements`", "`Exclusion list`" and "`Output BOM`", in different groups,
  start and end at the same place, and are narrower once the window is, and that each of the "`Move up`",
  "`Move down`" and "`Remove`" buttons of a list's rows are lined up (`VBGUI-2`, `VBGUI-8`).
- `tests/ui/variant_builders.rs` checks the placements are chosen once there's an EDA tool, and again once it's
  changed, that the design is named after the placements until the user names it, and that "`Ok`" creates
  `<design name>_<variant name>.variant.json` (`VBGUI-1`).
- `tests/ui/variant_builders.rs` opens a design variant with "`Open design variant`" and checks each field shows what
  its file says (`VBGUI-2`), adds global and design substitutions, removes one, and checks the file is saved with the
  rest (`VBGUI-3`), adds substitutions and checks the table of them as they're applied has the rows
  "`Order`, `File`, `Source`", then "`1`, `d1.csv`, `Design`" and so on, the design's first, then moves one down and
  up, checking the first's "`Move up`" and the last's "`Move down`" are disabled, the table follows, and the file is
  saved in the new order, logging "`MoveDownClicked(DesignSubstitutions, 0)`" and
  "`MoveUpClicked(DesignSubstitutions, 1)`" (`VBGUI-2`, `VBGUI-7`), creates assembly rules with
  "`Assembly rules: New`", checking the file and the suggested name are as `VBGUI-6` says, and the variant builder's assembly rules are set to it (`VBGUI-6`), checks a new design variant whose
  file exists isn't created, the tab says "`Variant builder exists already.`" and the file is kept (`VBGUI-1`), and
  that building one that isn't complete says what it's missing (`VBGUI-4`).
- Not covered yet: restoring a variant builder's tab when the GUI starts again (`VBGUI-5`), as the GUI's tests start
  with no saved state, and the changes kept when the core refuses them (`VBGUI-3`), as the GUI's tests can't make the
  file unwritable between it being read and written.
