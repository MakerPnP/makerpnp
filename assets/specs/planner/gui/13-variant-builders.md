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

**VBGUI-9 — Panels.** *Confirmed.* A design variant's tab, once it has a file, MUST show its panels, docked, as a
project's are: "`Explorer`" (`VBGUI-10`), on the left, which can't be closed, and "`Overview`", beside it, which has
the fields `VBGUI-2` to `VBGUI-8` describe, and the other panels the explorer opens. A panel that's open is shown
rather than opened again, and a panel is opened beside the explorer, with the panels other than the explorer. The
panels open MUST be kept, and shown again when the GUI starts again (`VBGUI-5`); with none, the tab MUST show the
explorer and the "`Overview`".

**VBGUI-10 — The explorer.** *Confirmed.* The "`Explorer`" panel MUST show a tree of the variant builder, as the
"`Overview`" shows it, with the changes not yet saved, each node showing its panel, opening it if it isn't open, when
it's double-clicked (`VBGUI-9`): the root, named after the variant builder's file, and an "`Overview`" node under it,
showing the "`Overview`"; the placements, as "`Placements: <file>`"; a folder for each of the "`Overview`"'s groups,
"`Part matching`" and "`Design specific`", named as the groups are, showing the "`Overview`", with a node for each of
its data sources, "`<field>: <file>`", or "`<field>: (none)`" if it isn't set, which shows nothing, and a folder for
each list of files, named after the field, showing the "`Overview`", where the list is changed, with a node for each
file, in order, its path as in its list; and a "`Build output`" folder, showing the "`Build output`" panel, with the
nodes "`Placements`" and "`BOM`" (`VBGUI-12`). Each folder is open to begin with.

**VBGUI-11 — A data source's panel.** *Confirmed.* A data source's node MUST open a panel named "`<field>: <file>`",
showing "`File: <file>`", and its records as a table, read-only, with the stores core (`STORE-17`), which takes all of
the panel's space left below what's above it, ending at the panel's bottom, so its last rows can be scrolled to
(`ISSUE-25`), and scrolls itself, resized with the panel. Each column is named as `STORE-17` names it, in its
order, a comment's last, and MUST be resizable, by dragging the edge on its right, which stays under the pointer, the last
filling the rest of the width. Each value is shown as what it is (`STORE-17`): a pattern in blue if it's a value matched
exactly, in green if it's a valid regular expression, and in red if it isn't, saying why when it's hovered, as it was
parsed when it was read, not as it's shown. The placements are read in the variant builder's EDA tool's format, and
parts are shown as the parts view shows them (`STORE-2`). A data source is opened once, however often its panel is shown, and is watched, so its table follows its
file (`STORE-9`). If the stores core refuses it, the panel MUST say why, such as "`Data source not found.`", with the
table as it was last read. *Observed*: the tab's "`ShowPanel(<panel>)`", "`OpenTable(<kind>, <source>)`" and
"`OpenParts(<source>)`" commands are logged.

**VBGUI-12 — The build's output.** *Confirmed.* The "`Build output`" panel is where the mapping results are to be
shown, and until then MUST say "`The mapping results are to come.`" (the Variant Builder mission's step 10e). Its
"`Placements`" and "`BOM`" nodes MUST open panels named so, showing the variant's placements and BOM as tables, as
`VBGUI-11` does, from where building writes them (`VARB-9`), as the file says, without the changes not yet saved. Until
the variant is built, the panel says "`Data source not found.`", and once it's built it's shown, as the output is
watched (`VBGUI-11`).

**VBGUI-13 — Changing part mappings.** *Confirmed.* A part mappings' panel MUST change its data source where it's
shown, with the stores core (`STORE-19`), each change written at once, so nothing is left unsaved. A cell is edited, in
place, by clicking it, then clicking it again: a pattern, a manufacturer or an MPN in a text field, a pattern's text
coloured by what it is as it's typed, as `VBGUI-11` colours it, and the EDA tool chosen from a list. The edit is written
once it's finished, by pressing Enter or clicking elsewhere, if the value changed; pressing Escape leaves the value as it
was. A row MUST be selected, and only it, when any of its cells is clicked, or, with Ctrl, selected or deselected,
keeping the others, as `egui_deferred_table` selects them; clicking a row's number selects or deselects it, keeping the
others, so one or more rows are selected. Above the table, "`Add row`" MUST add a part mapping after the others, of the
variant builder's EDA tool, with no patterns and no part, to be filled in; "`Remove row`" MUST remove the selected row;
and "`Move up`" and "`Move down`" MUST move it before the row before it, or after the row after it. The row buttons are
disabled unless one row is selected, so several rows selected are neither moved nor removed, and while a cell is being
edited; "`Move up`" for the first row, "`Move down`" for the last, and both if the rows can't be moved (`STORE-19`); and
every button while a change is being made. Once a row button's change is shown, the table read again after it's
written, the row added MUST be selected, alone, and brought into view, a moved row MUST stay selected, alone, where
it's moved to, and brought into view, and once a row is removed, no row is selected, not the one now in its place; the
cell chosen for editing is no longer chosen, as it was of the row in its place before. A change that wasn't made, as
the table is the same, or whose row isn't in it, leaves the rows selected as they were. An edit leaves them as they
are. Each change is of the row as it was shown when the change was made, so a change made since, by anyone, is refused,
and the panel MUST say why, as the core does, until a change succeeds, showing the table as it is now (`STORE-19`).
Every panel showing the data source, in every variant builder, shows a change once it's written (`STORE-9`). The other
tables stay read-only, with no buttons, and their cells can't be edited. *Observed*: the tab's
"`Table(<kind>, <source>, <command>)`" commands are logged, a row button's as "`ChangeRows(<change>, <after change>)`",
and once it's shown, "`ChangeShown(Row(<place>))`", "`ChangeShown(NoRow)`" or "`ChangeShown(Unchanged)`", and, for a
row brought into view, "`ScrolledToRow`". A row of any EDA tool can be given its patterns, whichever tools' columns
the data source has. Choosing a row's EDA tool whose columns the data source doesn't have MUST first add them, as the
core works them out (`STORE-21`), then, once the table's read again, change the row's tool, as the row is shown then,
if it's still as it was; "`Add row`" likewise adds the variant builder's EDA tool's columns, if they're needed, then
the row, which is selected once it's shown, as a row added is. The row buttons are disabled until the row's change is
made. If adding the columns is refused, the panel says why (`STORE-21`), and the row is left as it was. Changing a
row's tool keeps its patterns for the other tools' fields, as they're the user's. Every pattern's cell is shown, and
editable, whichever tool the row has, and its background, faint, so the pattern's own colour can still be read, says
how the row's tool uses it, the first of these that's so:

- **Blue**: the row's tool doesn't match the field, so its pattern is ignored, saying "`Not matched by <tool>.`".
- **Red**: the tool matches the field, and its pattern is a regular expression that isn't valid, saying
  "`Matched by <tool>.`", and why the expression isn't valid below it.
- **Orange**: the tool matches the field, and every one of the tool's fields' patterns in the row is empty, or the data
  source has no column for it, so the row has nothing to match with, saying
  "`Matched by <tool>. None of its fields has a pattern.`".
- **Green**: otherwise, the tool matches the field, saying "`Matched by <tool>.`".

The cell says so when it's hovered, even with no pattern, so the colour isn't the only sign. A manufacturer's, or an
MPN's, cell is likewise red if it's empty, as every part mapping needs both, and green if not. The cell's editor, shown
over it, isn't coloured.

The table's first column, shown only by the GUI, is the row's status, headed "`◑`", a circle half filled, depicting its
two states: a red circle, "`⏺`", if the row has any of these problems, and a green one if it has none. Hovering it says
what each problem is, in this order, or "`No problems.`":

- "`A pattern isn't valid.`": one of the row's patterns, of any field, as the user may give the row another tool, is
  a regular expression that isn't valid.
- "`The data source has no column for one of <tool>'s fields.`": the table lacks a column of one of the row's tool's
  fields (`STORE-21`).
- "`None of <tool>'s fields has a pattern.`": every one of the tool's fields' patterns in the row is empty, or the data
  source has no column for it, as an orange cell says.
- "`The data source has no manufacturer's column, or no MPN's.`".
- "`The manufacturer, or the MPN, is empty.`": said only if the data source has both columns.

*Target*: a value of only whitespace, or other characters that aren't printed, such as one or more spaces, is a
problem, as an empty one is, in every changeable table's status and cells, part mappings', substitutions'
(`VBGUI-14`) and assembly rules' (`VBGUI-15`), as it's of no use to MakerPnP or the user. The gap: such a value is
valid now, as any other that isn't empty is.

A row's status's text, and its EDA tool's, MUST NOT be selectable, as they're shown, or chosen, not typed, so dragging
across them doesn't select them; the other cells' text can be selected, to be copied, as a read-only table's can.
The circles are glyphs of egui's own fonts, tinted, as they have no colour emoji, nor a traffic light. *Target*: colour
emoji, once the GUI uses an egui with colour fonts. The gap: egui 0.36 has none.

*Observed*: the tab's
"`ChangeAfterColumns(<pending change>)`" command is logged, then the core's "`AddTableColumns`" event, and once the
columns are shown, "`ColumnsShown(Some((<change>, <after change>)))`", before the row's "`ChangeTable`", or
"`ColumnsShown(None)`" if the columns weren't added, or the row isn't as it was. *Target*: several rows selected are
removed at once. The gap: the stores core makes one change at a time.

**VBGUI-14 — Changing substitutions.** *Confirmed.* A substitutions' panel, design or global, MUST change its data
source as a part mappings' panel does (`VBGUI-13`): its cells edited in place, its rows added, removed and moved, a
row's EDA tool chosen, its columns added first if they're needed (`STORE-21`), each change written at once, and the
table's first column its rows' status. A pattern, a value a field is substituted with, and the comment are edited in a
text field, and "`Add row`" adds a substitution of the variant builder's EDA tool, with no patterns, values or comment.
Changing a row's tool keeps its patterns and values for the other tools' fields. A pattern's cell is coloured, and says
so when it's hovered, as a part mapping's is (`VBGUI-13`). A value's cell says whether it's used to substitute the
row's tool's values, as MakerPnP substitutes them, not the tool, the first of these that's so:

- **Blue**: the field isn't one of the row's tool's, so its value isn't used, saying
  "`Not used to substitute <tool> values.`".
- **Red**: the field is one of the tool's, and every one of the tool's fields' values in the row is empty, or the data
  source has no column for it, saying "`Used to substitute <tool> values. Substitution values not defined.`".
- **Green**: otherwise, saying "`Used to substitute <tool> values.`", an empty value being one, as a field can be
  substituted with nothing.

The comment isn't coloured. A row's status is red if it
has any of these problems, in this order, and green, saying "`No problems.`", if not:

- "`A pattern isn't valid.`": one of the row's patterns, of any field, is a regular expression that isn't valid.
- "`The data source has no column for one of <tool>'s fields.`": the table lacks a pattern's or a value's column of one
  of the row's tool's fields, so the Variant Builder's build refuses it (`STORE-17`).
- "`None of <tool>'s fields has a pattern.`": every one of the tool's fields' patterns in the row is empty, or the data
  source has no column for it, so it matches only placements whose fields are all empty.
- "`Substitution values not defined.`": every one of the tool's fields' values in the row is empty, or the data
  source has no column for it, as a red value's cell says.
- "`Matching patterns and values have no effect.`": every one of the tool's fields' patterns is a value matched
  exactly, the same as the value it's substituted with, a missing pattern or value being empty, so it never changes a
  placement. A row with no patterns and no values, such as one just added, has this problem and the two before.

The table's columns are its rows' EDA tool, then every pattern's column, then every value's column, each group in the
EDA tools' order, then the comment, as the core shows them (`STORE-17`), whatever their order in the data source.

*Observed*: the tab's commands are logged as `VBGUI-13` says, with the kind "`Substitutions`".

**VBGUI-15 — Changing assembly rules.** *Confirmed.* An assembly rules' panel MUST change its data source as a part
mappings' panel does (`VBGUI-13`): its cells edited in place, its rows added, removed and moved, each change written at
once, and the table's first column its rows' status. A designator, a manufacturer and an MPN are edited in a text
field. An assembly rule has no EDA tool, so it has no EDA tool's cell, and no columns are added for one, and
"`Add row`" adds a rule with no designator, manufacturer or MPN, whether or not the variant builder has an EDA tool.
Each of a row's cells is red if it's empty, and green if not, as every assembly rule needs all three. A row's status
is green, saying "`No problems.`", if none of its cells is empty, and red if any is, saying, for each, in the columns'
order:

- "`Empty RefDes.`"
- "`Empty Manufacturer.`"
- "`Empty Mpn.`"

So an empty row, such as one just added, says all three. A row's status's text MUST NOT be selectable, as a part
mapping's isn't (`VBGUI-13`). *Observed*: the tab's commands are logged as `VBGUI-13` says, with the kind
"`AssemblyRules`".

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
- `tests/ui/variant_builders.rs` checks a design variant has the "`Explorer`" and "`Overview`" panels, that the
  explorer shows the variant builder's file, "`Overview`", "`Placements: board1.csv`", "`Parts: parts.csv`", chosen
  but not yet saved, "`Part mappings: (none)`", "`Assembly rules: (none)`", "`Build output`" and "`BOM`", the
  folders "`Part matching`", "`Global substitutions`", "`Design specific`" and "`Design substitutions`", as the
  "`Overview`" names them, and a global substitution added but not yet saved (`VBGUI-9`, `VBGUI-10`).
- `tests/ui/variant_builders.rs` double-clicks the part mappings, the parts and the placements, checking each panel is
  shown, in place of the "`Overview`", with its table's columns and rows, and that double-clicking "`Overview`" and the
  variant builder's file shows the "`Overview`" again, and checks the tab's "`ShowPanel(DataSource { source: PartMappings, `"
  and "`OpenTable(PartMappings, `", "`stores_app: Received event. event: OpenTable { kind: PartMappings, `",
  "`stores_service: Performing stores operation. operation: ReadTable { kind: PartMappings, `",
  "`stores_app: Loaded table. kind: PartMappings, `", the tab's "`ShowPanel(Overview)`", "`OpenParts {`",
  "`stores_app: Loaded parts.`" and "`stores_app: Loaded table. kind: EdaPlacements { eda_tool: DipTrace }, `" are
  logged, in order, with no "`Event failed.`" (`VBGUI-10`, `VBGUI-11`).
- `tests/ui/variant_builders.rs` double-clicks "`Build output`", checking it says the mapping results are to come, then
  "`BOM`", checking it says "`Data source not found.`", then builds the variant, and checks the BOM's panel shows its
  columns and rows, and that the tab's "`ShowPanel(BuildOutput)`" and "`ShowPanel(Output(Bom))`",
  "`stores_app: Received event. event: OpenTable { kind: Bom, `", "`stores_app: Event failed. error: DataSourceNotFound {`",
  the tab's "`BuildClicked`", "`variantbuilder_app: Built variant.`",
  "`stores_app: Received event. event: TableSourceChanged { kind: Bom, `" and "`stores_app: Loaded table. kind: Bom, `"
  are logged, in order (`VBGUI-12`).
- `tests/ui/variant_builders.rs` checks the "`Overview`"'s fields are narrower once the window is, as it's beside the
  explorer (`VBGUI-8`, `VBGUI-9`).
- `tests/ui/variant_builders.rs` opens part mappings with an exact pattern, a valid regular expression and one that
  isn't, checks each is shown, as the data source has it, its columns in order, and that hovering the one that isn't
  valid says "`Matched by DipTrace.`", as its cell is red, and "`regex parse error`" (`VBGUI-13`), and checks a BOM's
  comment is its last column. `data_sources::table`'s tests check
  a pattern's colour, in the dark and light themes, as the GUI's tests can't see colours (`VBGUI-11`).
- `tests/ui/variant_builders.rs`'s `a_data_sources_columns_can_be_resized` drags the edges between part mappings'
  columns and checks each column is as much wider as the edge was dragged; it's the regression test for `ISSUE-24`
  (`VBGUI-11`).
- `tests/ui/variant_builders.rs` edits a part mapping's pattern in its cell, finished by clicking another row, and its
  MPN, finished by pressing Enter, chooses its EDA tool, EasyEDA, whose "`Device`" column is added first, and checks
  the file after each, and checks the tab's
  "`Table(PartMappings, `", "`stores_app: Received event. event: ChangeTable {`",
  "`stores_service: Performing stores operation. operation: ChangeRows {`" and
  "`stores_app: Changed table. kind: PartMappings, `" are logged, in order, with no "`Event failed.`" (`VBGUI-13`).
- `tests/ui/variant_builders.rs`'s `a_part_mappings_rows_are_added_moved_and_removed` selects a row by clicking one of
  its cells, adds a row, checking it's selected in its place, fills in its pattern, moves it up twice, checking it stays
  selected where it's moved to, selects a second row by its number, checking the row buttons are disabled with two rows
  selected, clicks a cell, checking only its row is then selected, moves it down, and removes a row in the middle,
  checking no row is selected after, checking "`Move up`" is disabled for the first row and "`Move down`" for the last,
  and the file after each. It checks "`ChangeRows(Add {`", "`stores_app: Received event. event: ChangeTable {`",
  "`stores_app: Changed table. kind: PartMappings, `", "`stores_app: Loaded table. kind: PartMappings, `",
  "`ChangeShown(Row(2))`", "`ScrolledToRow`", "`ChangeRows(Remove {`" and "`ChangeShown(NoRow)`" are logged, in
  order, with no "`Event failed.`", and that the placements' panel has no row buttons, and its cells can't be edited
  (`VBGUI-13`).
- `tests/ui/variant_builders.rs`'s `a_row_added_is_brought_into_view_and_selected` adds a row to 200 part mappings, and
  checks the row added, the 201st, is shown, above the table's horizontal scroll bar, and selected, and that
  "`ChangeRows(Add {`", "`stores_app: Loaded table. kind: PartMappings, `", "`ChangeShown(Row(200))`" and
  "`ScrolledToRow`" are logged, in order (`VBGUI-13`).
- `data_sources::table`'s tests check an edit makes the change of the row as it was, with the value as it's to be, that
  one leaving the value as it was, or abandoned, makes none, that the rows selected are kept as `egui_deferred_table`
  says, and an edit leaves them, that once a row button's change is shown its row is selected, nearest the place it's
  to be, the one added read back with an empty pattern of each field, or no row, after a remove, and they're kept if
  the change wasn't made, and that the row is brought into view once, and the cell chosen is no longer chosen
  (`VBGUI-13`).
- `data_sources::table`'s tests check an edit giving a row an EDA tool whose columns the table doesn't have asks for
  them to be added, the change of its tool pending, and one whose columns it has is made at once, that once the columns
  are shown the pending change is made of the row as it's shown then, keeping its patterns, or not if the row changed
  or is gone, that a row added is added and selected once it's shown, that nothing is made if the columns weren't
  added, and that a pattern's cell is blue if the row's tool doesn't match its field, whatever its pattern, red if it
  does and its pattern is a regular expression that isn't valid, orange if it does and every one of the tool's fields'
  patterns in the row is empty or missing, and green otherwise, a field shared by tools matched by each, each faint,
  and each colour told apart from the others, in either theme, that a manufacturer's or an MPN's cell is red if it's
  empty and green if not, that a part mapping's problems are each found, in order, and none if it has none, that the
  status column is first only of part mappings and substitutions, and that its header and circle are glyphs egui's
  own fonts have (`VBGUI-13`, `VBGUI-14`).
- `tests/ui/variant_builders.rs`'s `a_part_mappings_status_says_what_its_problems_are` checks the status column,
  "`◑`", is before "`Eda`", and hovers each row's status, checking "`No problems.`" for a row with none,
  "`A pattern isn't valid.`" and "`The manufacturer, or the MPN, is empty.`" for a row with an invalid pattern and no
  MPN, and "`The data source has no column for one of KiCad's fields.`" and "`None of KiCad's fields has a pattern.`"
  for a KiCad row of DipTrace's columns (`VBGUI-13`).
- `tests/ui/variant_builders.rs`'s `choosing_a_rows_eda_tool_adds_its_columns_first` chooses KiCad for a row of
  DipTrace's part mappings, checking "`Package`" and "`Val`" are added after "`Value`", the last pattern's column, then the row's tool is
  changed, keeping its patterns, then EasyEDA, checking only "`Device`" is added, then KiCad again, whose columns are
  there, checking the file after each, and hovers pattern cells, checking "`Not matched by KiCad.`" and
  "`Not matched by DipTrace.`" for fields the row's tool doesn't match, "`Matched by KiCad. None of its fields has a
  pattern.`" for the KiCad row, whose KiCad patterns are empty, and "`Matched by DipTrace.`" for a DipTrace row's
  pattern are shown. It checks "`ChangeAfterColumns(`",
  "`stores_app: Received event. event: AddTableColumns {`", the core's "`Added table columns.`",
  "`stores_app: Loaded table. kind: PartMappings, `", "`ColumnsShown(Some((Update {`",
  "`stores_app: Received event. event: ChangeTable {`" and "`stores_app: Changed table. kind: PartMappings, `" are
  logged, in order, with no "`Event failed.`". `adding_a_row_adds_its_eda_tools_columns_first` adds a DipTrace row to
  KiCad's part mappings, checking "`Name`" and "`Value`" are added first, and the row selected, and logs
  "`ColumnsShown(Some((Add {`" and "`ChangeShown(Row(1))`". `a_rows_tool_whose_columns_can_not_be_added_is_left_as_it_was`
  makes the file read-only, chooses KiCad, and checks the panel says "`Unable to change the table.`", the file and the
  row are as they were, "`ColumnsShown(None)`" is logged, and no "`ChangeTable`", and the row buttons work again
  (`VBGUI-13`, `STORE-21`).
- `tests/ui/variant_builders.rs`'s `a_table_too_big_for_its_panel_ends_within_it` shows 200 part mappings, and 200
  parts, scrolls each table to its end, and checks its last row is shown above the table's horizontal scroll bar; it's
  the regression test for `ISSUE-25` (`VBGUI-11`).
- `tests/ui/variant_builders.rs`'s `a_rows_status_and_eda_tool_can_not_be_selected` drags across a row's status, its
  EDA tool and another cell, then copies, checking nothing is copied of the status or the EDA tool, and the other
  cell's text is, in a part mappings' table and a substitutions' (`VBGUI-13`, `VBGUI-14`).
- `tests/ui/variant_builders.rs`'s `a_design_substitutions_values_are_edited_and_its_rows_changed` edits a design
  substitution's pattern, value and comment in their cells, adds, moves and removes rows, chooses a row's EDA tool,
  KiCad, whose columns are added first, and checks the file after each, hovers a value's cell of a field of the row's
  tool, and of one that isn't, checking "`Used to substitute DipTrace values.`" and
  "`Not used to substitute KiCad values.`",
  and checks "`Table(Substitutions, `",
  "`stores_app: Received event. event: ChangeTable { kind: Substitutions, `" and
  "`stores_app: Changed table. kind: Substitutions, source: '`" are logged, in order, with no "`Event failed.`".
  `a_global_substitutions_status_says_what_its_problems_are` hovers a global substitutions' rows' statuses, checking
  "`No problems.`", "`A pattern isn't valid.`", "`None of DipTrace's fields has a pattern.`" and
  "`Matching patterns and values have no effect.`" and "`Substitution values not defined.`", and a value's cell of a
  row with no values, checking "`Used to substitute DipTrace values. Substitution values not defined.`", then changes
  a row's value, so it changes something, checking the file, that a global substitutions' panel can be changed too
  (`VBGUI-14`).
- `data_sources::table`'s tests check a substitution's value's cell is blue if the row's tool doesn't substitute its
  field, red if it does and every one of the tool's values is empty, and green otherwise, an empty value too, that a
  substitution's pattern, value and comment are edited, one
  left empty not being one the row didn't have, and that a substitution's problems are each found, in order, and
  none if it has none (`VBGUI-14`).
- `tests/ui/variant_builders.rs` edits an assembly rule's designator, manufacturer and MPN in their cells, adds,
  moves and removes rows, checking the file after each, hovers rows' statuses, checking "`No problems.`" for a row
  with none, "`Empty Mpn.`" for a row with no MPN, and "`Empty RefDes.`", "`Empty Manufacturer.`" and "`Empty Mpn.`"
  for the row added, and that a status can't be selected, and checks "`Table(AssemblyRules, `",
  "`stores_app: Received event. event: ChangeTable { kind: AssemblyRules, `" and
  "`stores_app: Changed table. kind: AssemblyRules, source: '`" are logged, in order, with no "`Event failed.`"
  (`VBGUI-15`).
- `data_sources::table`'s tests check an assembly rule's cells are red if empty and green if not, that its problems
  are each found, in the columns' order, and none if it has none, that a new row is an empty rule, and that the status
  column is first of assembly rules too (`VBGUI-15`).
- Not covered yet: restoring a variant builder's tab when the GUI starts again (`VBGUI-5`), and its panels
  (`VBGUI-9`), as the GUI's tests start with no saved state, and the changes kept when the core refuses them
  (`VBGUI-3`), as the GUI's tests can't make the file unwritable between it being read and written.
