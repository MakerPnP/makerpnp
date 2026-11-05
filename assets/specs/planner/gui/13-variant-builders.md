# GUI 13 — Variant builders

A variant builder's tab, in which the user defines a variant, chooses the data sources it's built from, saves them to
its file, and builds it, through the Variant Builder's core (`../../variantbuilder/core/01-variant-builders.md`), as
the CLI's `create`, `configure` and `build` do (`VARBCLI`). The variant builder's explorer, and the tabs for its data
sources and results, are to come (`../../../missions/variant-builder.md`, step 10).

**VBGUI-1 — New variant builder.** *Confirmed.* "`New variant builder`" in the toolbar MUST open a "`New variant
builder`" tab, with "`Variant builder name`" and "`Directory`", chosen with a dialog, both required before "`Ok`" is
enabled. "`Ok`" MUST create `<name>.variant.json` in the directory (`VARB-2`), without replacing a file that's there,
and the tab MUST then be the variant builder's, named after its file. If the core refuses, the tab MUST show why.

**VBGUI-2 — Open variant builder.** *Confirmed.* "`Open variant builder`" in the toolbar MUST have the user choose a
variant builder's file with a dialog, and open it in a tab of its own (`VARB-3`), named after its file, which shows
what the file says: "`Variant name`", "`Ref-des list`", "`EDA tool`", "`Placements`", "`Parts`",
"`Part mappings`", "`Load-out`", "`Assembly rules`", "`Substitutions`", "`Ref-des excluded`",
"`Ref-des not placed`", "`Output placements`" and "`Output BOM`". Each ref-des list is shown, and typed, separated by
commas.

**VBGUI-3 — Changing a variant builder.** *Confirmed.* The user MUST be able to change each field: the text fields by
typing, "`EDA tool`" from a list, and each data source by choosing its file with "`<field>: Choose`", or clearing it
with "`<field>: Clear`". "`Substitutions: Add`" MUST add the files chosen to the substitutions. A file chosen MUST be
kept relative to the variant builder's folder, or absolute if it's outside it (`VARB-1`). The changes MUST be shown at
once, and kept by the tab until "`Save changes`", enabled once a field differs from what the file says, configures the
variant builder with each field that differs (`VARB-4`). The tab MUST keep the changes if the core refuses them, so the
user doesn't lose them (`FORM-5`). The toolbar's "`Save`" doesn't save a variant builder.

**VBGUI-4 — Building.** *Confirmed.* "`Build`" MUST build the variant builder's variant (`VARB-5`), and so is enabled
only while there are no changes to save, as the variant is built from what the file says, and while nothing else is
being done to the variant builder (`VARB-7`). The tab MUST show a spinner while the core is busy with the variant
builder, "`Built.`" once its variant is built, until it's changed, and why the core refused its last event, if it did.

**VBGUI-5 — Restoring.** *Observed.* A variant builder's tab MUST be shown again when the GUI starts again, and a tab
with a file MUST open it again (`VARB-3`).

## Acceptance criteria

- `tests/ui/variant_builders.rs` creates a variant builder with "`New variant builder`", chooses its EDA tool and data
  sources, which are shown relative to its folder, types its variant's name, checks "`Build`" is disabled until
  "`Save changes`" is clicked, and that the file is saved, then builds it, and checks "`Built.`" is shown and the
  placements and BOM are written. It checks "`planner_gui_egui::toolbar: Toolbar command. command: NewVariantBuilderClicked`",
  the tab's "`Submit`", "`variantbuilder_app: Received event. event: Create {`", the stores service's "`ReadDocument`"
  and its reply as a "`VariantBuilderReply`" command, "`WriteDocument`",
  "`variantbuilder_app: Created variant builder. path: '`", the tab's "`Changed(EdaTool(Some(DipTrace)))`",
  "`PickClicked(Placements)`", "`FilesPicked(Placements, [`" and "`SaveClicked`",
  "`variantbuilder_app: Received event. event: Configure {`", "`variantbuilder_app: Configured variant builder.`",
  the tab's "`BuildClicked`", "`variantbuilder_app: Received event. event: Build {`",
  "`variantbuilder_app::variant_builders: Building variant.`" and "`variantbuilder_app: Built variant.`" are logged,
  in order, with no "`Event failed.`" (`VBGUI-1`, `VBGUI-3`, `VBGUI-4`).
- `tests/ui/variant_builders.rs` opens a variant builder with "`Open variant builder`" and checks each field shows
  what its file says (`VBGUI-2`), that a new variant builder whose file exists isn't created, the tab says
  "`Variant builder exists already.`" and the file is kept (`VBGUI-1`), and that building one that isn't complete
  says what it's missing (`VBGUI-4`).
- Not covered yet: restoring a variant builder's tab when the GUI starts again (`VBGUI-5`), as the GUI's tests start
  with no saved state, and the changes kept when the core refuses them (`VBGUI-3`), as the GUI's tests can't make the
  file unwritable between it being read and written.
