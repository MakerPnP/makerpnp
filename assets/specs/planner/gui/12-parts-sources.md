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

**PARTSRC-3 — Showing the parts.** *Confirmed.* "`Ok`" MUST have the stores core open the data source (`STORE-5`), and
the tab MUST then show its parts in a table (`STORE-2`). *Observed*: the tab is then labelled with the file's name, and
shows "`CSV file: <path>`", the file's canonical path once the stores service has identified it (`STORE-6`), and
"`Reload`". The table's columns are "`Manufacturer`", "`MPN`", and then a column for each of the parts' other values,
named as in the file. If the core refuses the data source, the tab MUST show the core's error, which names a missing
header (`STORE-3`). The tab MUST show the file's parts once it's fixed, or once it's changed, without "`Reload`", as the
data source is watched (`STORE-9`), even if it couldn't be read. "`Reload`" MUST load the data source again (`STORE-1`),
for a data source whose watch was refused, or whose change wasn't noticed. The error is the data source's: every tab showing it MUST show the error, and keep
showing the parts loaded before, if any. A data source with no parts MUST say "`There are no parts.`", with no table
(`FORM-8`). The GUI doesn't wait for the stores service (`STORE-7`), so until the parts are read the tab says
"`Loading the parts...`".

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

**PARTSRC-6 — Adding and removing parts.** *Confirmed.* Once its parts are shown, the tab MUST have "`Manufacturer`"
and "`MPN`", for the part to add or remove, "`Add part`", which has the stores core add the part, after the others,
and "`Remove part`", which has it remove the part (`STORE-15`). Each change MUST be based on the revision of the parts
the tab shows, so it's refused if the data source has changed since, and every tab showing the data source MUST then
show the change, or, if it's refused, why, until a change succeeds, as the error is the data source's (`PARTSRC-5`).
*Observed*: "`Add part`" is enabled only for a part, both its manufacturer and its MPN typed, that isn't shown, and
"`Remove part`" only for one that is, and neither while a change to the data source is being written. A part added has
no other values; they're empty, as each part has a value of every name. The part typed is kept once it's added or
removed.

**PARTSRC-7 — Changing a part's other values.** *Confirmed.* A parts tab's table MUST edit a part's other values in
their cells, as a variant builder's changeable tables edit theirs (`VBGUI-13`): a cell clicked once is chosen, and
clicked again is edited, in a text field, the edit finished with Enter, or by clicking elsewhere, and abandoned with
Escape, leaving the value as it was. A finished edit that changes the value MUST be written at once, the stores core
setting only that value of the part, keeping its others (`STORE-15`), an empty value too, as each part has a value of
every name. The change MUST be based on the revision of the parts shown when the edit began, so it's refused if the
data source has changed since, the refusal shown as `PARTSRC-6`'s are, and the change's text lost, as the value shown
is the data source's. A part's manufacturer and MPN MUST NOT be edited, as they name the part; a part is renamed by
removing it and adding it again (`PARTSRC-6`). *Target*: renaming a part in its cells. The gap: `STORE-15` has no
change for it. An edit MUST NOT begin while a change to the data source is being written, as it would be based on
the parts before it, and so refused. *Observed*: the tab's commands are logged as
"`PartsTab command. command: Table(<command>)`", such as "`Table(EditSubmitted)`", or "`Table(Change(PartsEdit {`" for
an edit finished by clicking elsewhere. A variant builder's parts panel stays read-only for now.

## Acceptance criteria

- `tests/ui/parts_sources.rs` opens a CSV file using "`Open parts`", and checks the tab's label, the columns, and each
  value, including values that look like numbers, and that the toolbar's and the tab's commands, ending with
  "`PartsTab command. command: Open`", then "`stores_app: Received event. event: OpenParts {`", the stores service's
  "`ResolveSource`" and its "`StoresReply`", "`stores_app: Received event. event: SourceResolved {`", the service's
  "`ReadParts`" and its "`StoresReply`", "`stores_app: Received event. event: PartsRead {`" and
  "`stores_app: Loaded parts. source: '`", are logged, in order, with no "`Event failed.`" (`PARTSRC-1` to `PARTSRC-3`,
  `PARTSRC-5`, `ISSUE-23`). It then clicks "`Reload`", with the file unchanged, and checks
  "`PartsTab command. command: ReloadClicked`", "`stores_app: Received event. event: LoadParts {`",
  "`stores_app: Received event. event: PartsRead {`" and "`stores_app: Parts unchanged. source: '`" are logged, in
  order, with no "`Loaded parts.`", and that the parts are still shown (`STORE-8`).
- `tests/ui/parts_sources.rs` checks "`Data source`" starts at "`--choose source--`", that "`Ok`" is disabled until a CSV
  file is chosen, that cancelling the file dialog chooses nothing, that choosing "`--choose source--`" hides "`File`" and
  disables "`Ok`", that choosing "`CSV file`" again shows the file chosen before, and that "`File: Clear`" disables
  "`Ok`" (`PARTSRC-1`, `PARTSRC-2`, `FORM-5`).
- `tests/ui/parts_sources.rs` opens a file without the "`Manufacturer`" header, checks the error says so and that
  "`stores_app: Received event. event: OpenParts {`" and "`stores_app: Event failed. error: UnableToLoadParts {`" are
  logged, in order, then fixes the file, and checks the parts are shown and the error isn't, without "`Reload`", and
  that "`stores_app: Data source changed. source: '`" and "`stores_app: Loaded parts. source: '`" are logged, in order.
  It then clicks "`Reload`", and checks "`PartsTab command. command: ReloadClicked`",
  "`stores_app: Received event. event: LoadParts {`" and "`stores_app: Parts unchanged. source: '`" are logged, in
  order (`PARTSRC-3`, `STORE-3`, `STORE-9`).
- `tests/ui/parts_sources.rs` opens a file with only the headers, and checks "`There are no parts.`" is shown, with no
  table (`PARTSRC-3`).
- `tests/ui/parts_sources.rs` opens two data sources, checks each tab shows its own data source's parts, then opens one
  of them again in a third tab, with its path written with "`..`". It checks the path shown is the canonical one, and
  that "`PartsTab command. command: Open`", "`stores_app: Received event. event: OpenParts {`" and
  "`stores_app: Parts already loaded. source: '`" are logged, in order, with no "`Loaded parts.`", no second
  "`WatchSource`" and no "`Event failed.`". It then changes the file, as another process would, checks the changed parts
  are shown in the third tab, and that "`stores_app: Received event. event: SourceChanged {`",
  "`stores_app: Data source changed. source: '`" and "`stores_app: Loaded parts. source: '`" are logged, in order, and,
  after showing the other data source's tab, that the first tab shows them too, with no "`ReloadClicked`" and no
  "`LoadParts`" (`PARTSRC-5`, `STORE-5`, `STORE-6`, `STORE-9`).
- `tests/ui/parts_sources.rs` shows one data source in two tabs, makes the file invalid, clicks "`Reload`", and checks
  the error and the parts loaded before are shown, and logged, in order, then that the other data source's tab has no
  error, and that the first tab showing the data source shows the error and the parts loaded before
  (`PARTSRC-3`, `PARTSRC-5`, `STORE-3`).
- `tests/ui/parts_sources.rs` checks "`Add part`" and "`Remove part`" are disabled until a part is typed, that only
  "`Remove part`" is enabled for a part shown, and only "`Add part`" for one that isn't. It adds a part, checks only
  "`Remove part`" is then enabled, that "`PartsTab command. command: AddPartClicked`",
  "`stores_app: Received event. event: ChangeParts {`",
  "`stores_service: Performing stores operation. operation: WriteParts {`", its "`StoresReply`",
  "`stores_app: Received event. event: PartsWritten {`" and "`stores_app: Changed parts. source: '`" are logged, in
  order, with no "`Event failed.`", and that the file has the part, with empty values. It then removes it, and checks
  "`RemovePartClicked`" and "`Changed parts.`" are logged, in order, and that the file is as it was (`PARTSRC-6`,
  `STORE-15`).
- `tests/ui/parts_sources.rs`'s `a_parts_other_values_are_edited_in_their_cells` edits a part's other value, finished
  with Enter, an empty one, finished by clicking another part's cell, and a value left empty, checking the file after
  each, that a part's manufacturer and MPN can't be edited, and that "`Table(EditSubmitted)`",
  "`stores_app: Received event. event: ChangeParts {`", "`stores_app: Changed parts. source: '`" and
  "`Table(Change(PartsEdit {`" are logged, in order, with no "`Event failed.`" (`PARTSRC-7`, `STORE-15`).
- `data_sources::parts_table`'s tests check an edit finished with Enter sets only the part's value, based on the
  revision shown, keeping the cell chosen, that one leaving the value as it was, abandoned, or of another cell, changes
  nothing, and that one finished elsewhere is a change if it changes the value (`PARTSRC-7`).
- Not covered yet: restoring a parts tab when the GUI starts again (`PARTSRC-4`), as the GUI's tests start with no saved
  state, and "`Loading the parts...`", which is shown only until the stores service has read the file. A change refused
  as the file changed since, in the GUI (`PARTSRC-6`), as the watch reads the file again as soon as it's changed, so
  the GUI's tests can't change it between the parts being shown and the change; the core's tests check it.
