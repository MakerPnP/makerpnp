# GUI 12 — Parts data sources

Opening a parts data source, and showing its parts, through the stores core (`STORE` in `../core/04-stores.md`). The
first step of the mission to bring the Variant Builder into the GUI (`../../../missions/variant-builder.md`): data
sources are top-level, as projects, PCBs and processes are, so they can be used with no project or variant builder open.

These requirements were decided by the project's owner, those marked *Observed* are details of how they were done, and
have not been reviewed.

**PARTSRC-1 — Open parts.** *Confirmed.* "`Open parts`", on the application's toolbar, MUST open a tab, labelled
"`Open parts`", for choosing a data source. "`Data source`" MUST offer "`--choose source--`", which it shows at first, and
"`CSV file`". *Observed*: the data source is only read, so "`Save`" stays disabled while the tab is shown.

**PARTSRC-2 — Choosing a CSV file.** *Confirmed.* When "`CSV file`" is chosen, the tab MUST show a group, "`CSV file`",
with "`File`", chosen using a file dialog (`FORM-1` to `FORM-4`). *Observed*: "`Ok`" MUST be disabled until "`CSV file`"
and a file are chosen. Choosing "`--choose source--`" hides the group, and choosing "`CSV file`" again shows the file
chosen before (`FORM-5`).

**PARTSRC-3 — Showing the parts.** *Confirmed.* "`Ok`" MUST have the stores core open the data source's parts as a
table (`STORE-17`), and the tab MUST then show them with the GUI's one data source panel, as a variant builder's
"`Parts`" panel shows them, the same panel, buttons and table, which changes them (`VBGUI-16`). *Observed*: the tab is
then labelled with the file's name, and shows "`File: <path>`", the file's canonical path once the stores service has
identified it (`STORE-6`), and "`Reload`". The table's columns are "`Manufacturer`", "`Mpn`", and then a column for
each of the parts' other values, named as in the file, in its order (`STORE-17`). If the core refuses the data
source, the tab MUST show the core's error, which names a missing header (`STORE-3`). The tab MUST show the file's
parts once it's fixed, or once it's changed, without "`Reload`", as the data source is watched (`STORE-9`), even if it
couldn't be read. "`Reload`" MUST read the data source again (`LoadTable`, `STORE-17`), for a data source whose watch
was refused, or whose change wasn't noticed. The error is the data source's: every tab and panel showing it MUST show
the error, and keep showing the parts loaded before, if any. A data source with no parts shows the panel with
"`Add row`" enabled, so a part can be added (`VBGUI-16`); its table isn't drawn, not even its columns, as
`egui_deferred_table` draws nothing with no rows (`FORM-8`). The GUI doesn't wait for the stores service (`STORE-7`), so
until the parts are read the tab shows a spinner, as a variant builder's panel does.

**PARTSRC-4 — Restoring.** *Observed.* A parts tab MUST be shown again when the GUI starts again. A tab whose data source
had been chosen MUST open it again (`STORE-5`), so that tabs showing the same data source load it once.

**PARTSRC-5 — One core for every tab.** *Confirmed.* The GUI MUST have one stores core, used by every tab, so that a data
source can be shown in more than one tab, and is loaded, and watched, once however many tabs show it (`STORE-5`,
`STORE-6`, `STORE-9`). A change to a data source MUST be shown by every tab that uses it, once it's loaded again from
any of them, without each being reloaded. A change made outside the GUI, for example to the file by an editor, or by the
CLI, MUST be shown by every tab that uses it, without "`Reload`" (`STORE-9`). *Observed*: the GUI MUST keep one view of
each data source's parts, the latest the core sent, which every tab showing it shows, so that the tabs don't each keep a
copy of the parts. The GUI has one stores service too (`STORE-7`), whose operations are done on a runtime of their own,
and whose results come back as a command, "`UiApp command. command: StoresReply(`", handled in order with the others
(`GARCH-11`). A watch's results go on until the GUI closes, each one a "`StoresReply`".

**PARTSRC-6 — Changing parts.** *Confirmed.* Once its parts are shown, the tab MUST change them as a variant
builder's "`Parts`" panel does, with the same panel (`VBGUI-16`): every value, a part's manufacturer and MPN too,
edited in its cell, and parts added, removed and moved with "`Add row`", "`Remove row`", "`Move up`" and
"`Move down`", above the table, each change written at once, and every tab and panel showing the data source shows it
(`PARTSRC-5`). There are no "`Manufacturer`" and "`MPN`" fields above the table, nor "`Add part`" and
"`Remove part`". *Observed*: the tab's commands are logged as
"`PartsTab command. command: Panel(<command>)`", the panel's as `VBGUI-13` says.

## Acceptance criteria

- `tests/ui/parts_sources.rs` opens a CSV file using "`Open parts`", and checks the tab's label, the columns, and each
  value, including values that look like numbers, and that the toolbar's and the tab's commands, ending with
  "`PartsTab command. command: Open`", then "`stores_app: Received event. event: OpenTable { kind: Parts`", the stores
  service's "`ResolveSource`" and its "`StoresReply`", "`stores_app: Received event. event: TableSourceResolved {`",
  the service's "`ReadTable`" and its "`StoresReply`", "`stores_app: Received event. event: TableRead {`" and
  "`stores_app: Loaded table. kind: Parts, source: '`", are logged, in order, with no "`Event failed.`" (`PARTSRC-1` to
  `PARTSRC-3`, `PARTSRC-5`, `ISSUE-23`). It then clicks "`Reload`", with the file unchanged, and checks
  "`Panel(Reload(Parts, `", "`stores_app: Received event. event: LoadTable { kind: Parts`",
  "`stores_app: Received event. event: TableRead {`" and "`stores_app: Table unchanged. kind: Parts, source: '`" are
  logged, in order, with no second "`Loaded table.`", and that the parts are still shown (`STORE-8`, `STORE-17`).
- `tests/ui/parts_sources.rs` checks "`Data source`" starts at "`--choose source--`", that "`Ok`" is disabled until a CSV
  file is chosen, that cancelling the file dialog chooses nothing, that choosing "`--choose source--`" hides "`File`" and
  disables "`Ok`", that choosing "`CSV file`" again shows the file chosen before, and that "`File: Clear`" disables
  "`Ok`" (`PARTSRC-1`, `PARTSRC-2`, `FORM-5`).
- `tests/ui/parts_sources.rs` opens a file without the "`Manufacturer`" header, checks the error says so and that
  "`stores_app: Received event. event: OpenTable { kind: Parts`" and
  "`stores_app: Event failed. error: UnableToLoadTable {`" are logged, in order, then fixes the file, and checks the
  parts are shown and the error isn't, without "`Reload`", and that "`stores_app: Data source changed. source: '`" and
  "`stores_app: Loaded table. kind: Parts, source: '`" are logged, in order. It then clicks "`Reload`", and checks
  "`Panel(Reload(Parts, `", "`stores_app: Received event. event: LoadTable { kind: Parts`" and
  "`stores_app: Table unchanged. kind: Parts, source: '`" are logged, in order (`PARTSRC-3`, `STORE-3`, `STORE-9`).
- `tests/ui/parts_sources.rs` opens a file with only the headers, and checks "`Add row`" is enabled, and that adding a
  row shows the table, with its columns (`PARTSRC-3`).
- `tests/ui/parts_sources.rs` opens two data sources, checks each tab shows its own data source's parts, then opens one
  of them again in a third tab, with its path written with "`..`". It checks the path shown is the canonical one, and
  that "`PartsTab command. command: Open`", "`stores_app: Received event. event: OpenTable { kind: Parts`" and
  "`stores_app: Table already loaded. kind: Parts, source: '`" are logged, in order, with no second
  "`Loaded table.`", no second "`WatchSource`" and no "`Event failed.`". It then changes the file, as another process
  would, checks the changed parts are shown in the third tab, and that
  "`stores_app: Received event. event: TableSourceChanged {`", "`stores_app: Data source changed. source: '`" and
  "`stores_app: Loaded table. kind: Parts, source: '`" are logged, in order, and, after showing the other data
  source's tab, that the first tab shows them too, with no "`Panel(Reload(`" and no "`LoadTable`" (`PARTSRC-5`,
  `STORE-5`, `STORE-6`, `STORE-9`).
- `tests/ui/parts_sources.rs` shows one data source in two tabs, makes the file invalid, clicks "`Reload`", and checks
  the error and the parts loaded before are shown, and logged, in order, then that the other data source's tab has no
  error, and that the first tab showing the data source shows the error and the parts loaded before
  (`PARTSRC-3`, `PARTSRC-5`, `STORE-3`).
- `tests/ui/parts_sources.rs`'s `parts_are_changed_in_the_parts_tab` checks there are no "`Manufacturer`" and "`MPN`"
  fields, nor "`Add part`", adds a row, checks its cells are red and its status names each empty one, fills its
  manufacturer, MPN and other values in their cells, checking the file after each, and its status then says
  "`No problems.`", moves it up and removes it, checking the file is then as it was, and that
  "`PartsTab command. command: Panel(`", "`stores_app: Received event. event: ChangeTable { kind: Parts`",
  "`stores_service: Performing stores operation. operation: ChangeRows {`" and
  "`stores_app: Changed table. kind: Parts, source: '`" are logged, in order, with no "`Event failed.`" (`PARTSRC-6`,
  `VBGUI-16`, `STORE-19`).
- Not covered yet: restoring a parts tab when the GUI starts again (`PARTSRC-4`), as the GUI's tests start with no saved
  state, and the spinner, which is shown only until the stores service has read the file. A change refused
  as the file changed since, in the GUI (`PARTSRC-6`), as the watch reads the file again as soon as it's changed, so
  the GUI's tests can't change it between the parts being shown and the change; the core's tests check it.
