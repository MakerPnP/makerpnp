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
header (`STORE-3`). "`Reload`" MUST load the data source again (`STORE-1`), so that the tab shows the file's parts once
it's fixed, or once it's changed. The error is the data source's: every tab showing it MUST show the error, and keep
showing the parts loaded before, if any. A data source with no parts MUST say "`There are no parts.`", with no table
(`FORM-8`). The GUI doesn't wait for the stores service (`STORE-7`), so until the parts are read the tab says
"`Loading the parts...`".

**PARTSRC-4 — Restoring.** *Observed.* A parts tab MUST be shown again when the GUI starts again. A tab whose data source
had been chosen MUST open it again (`STORE-5`), so that tabs showing the same data source load it once.

**PARTSRC-5 — One core for every tab.** *Confirmed.* The GUI MUST have one stores core, used by every tab, so that a data
source can be shown in more than one tab, and is loaded once however many tabs show it (`STORE-5`, `STORE-6`). A change
to a data source MUST be shown by every tab that uses it, once it's loaded again from any of them, without each being
reloaded. *Observed*: the GUI MUST keep one view of each data source's parts, the latest the core sent, which every tab
showing it shows, so that the tabs don't each keep a copy of the parts. The GUI has one stores service too (`STORE-7`),
whose operations are done on a runtime of their own, and whose results come back as a command,
"`UiApp command. command: StoresReply(`", handled in order with the others (`GARCH-11`). *Target*: a change made outside the GUI, for
example to the file, or by the CLI, MUST be shown without "`Reload`". The gap: only "`Reload`" loads the data source
again.

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
  logged, in order, then fixes the file, clicks "`Reload`", and checks the parts are shown and the error isn't, and that
  "`PartsTab command. command: ReloadClicked`", "`stores_app: Received event. event: LoadParts {`" and
  "`stores_app: Loaded parts. source: '`" are logged, in order (`PARTSRC-3`, `STORE-3`).
- `tests/ui/parts_sources.rs` opens a file with only the headers, and checks "`There are no parts.`" is shown, with no
  table (`PARTSRC-3`).
- `tests/ui/parts_sources.rs` opens two data sources, checks each tab shows its own data source's parts, then opens one
  of them again in a third tab, with its path written with "`..`". It checks the path shown is the canonical one, and
  that "`PartsTab command. command: Open`", "`stores_app: Received event. event: OpenParts {`" and
  "`stores_app: Parts already loaded. source: '`" are logged, in order, with no "`Loaded parts.`". It then changes the
  file, clicks "`Reload`" in the third tab, checks the changed parts are shown, and, after showing the other data
  source's tab, that the first tab shows them too, with no "`Event failed.`", and no "`LoadParts`" but the third tab's
  (`PARTSRC-5`, `STORE-5`, `STORE-6`).
- `tests/ui/parts_sources.rs` shows one data source in two tabs, makes the file invalid, clicks "`Reload`", and checks
  the error and the parts loaded before are shown, and logged, in order, then that the other data source's tab has no
  error, and that the first tab showing the data source shows the error and the parts loaded before
  (`PARTSRC-3`, `PARTSRC-5`, `STORE-3`).
- Not covered yet: restoring a parts tab when the GUI starts again (`PARTSRC-4`), as the GUI's tests start with no saved
  state, and "`Loading the parts...`", which is shown only until the stores service has read the file.
