# Core 04 — Stores

The data sources the user's work draws on, such as their parts, and the core the front ends use them through, the
stores core (`planning/stores_app`). It is a core of its own, beside the planner's, because a data source is shared:
by many variant builders, by more than one window of a front end, and by every front end
(`../../../missions/variant-builder.md`).

A data source is a CSV file today. It may later be a database or another back end, so the front ends use a data source
only through the stores core, never by reading its files. The stores core is pure: it doesn't read the data sources
itself either, but asks its shell to, and the shell has the stores service (`planning/stores_service`) do it
(`STORE-7`).

The stores core logs each event it receives, and why it refused one, as the planner's core does (`LOG-1`, `LOG-2`), its
target being `stores_app`, for example "`stores_app: Received event. event: LoadParts {`". The stores service logs
each operation it's asked to do, its target being `stores_service`, for example
"`stores_service: Performing stores operation. operation: ReadParts {`".

These requirements were decided by the project's owner, those marked *Observed* are details of how they were done, and
have not been reviewed.

**STORE-1 — Loading parts.** *Confirmed.* The core MUST load the parts of a parts data source when asked
(`LoadParts`). Loading a data source again MUST replace the parts loaded from it before, so that it shows the data
source's current parts. *Observed*: the core MUST ask the stores service to identify the data source (`STORE-6`), then
to read its parts (`STORE-7`), and, once loaded, MUST send the data source's parts view (`STORE-2`), so that a front end
can update every view of the data source. A parts CSV file MUST have the headers "`Manufacturer`" and "`Mpn`", and its
other headers are the parts' other values, for example a supplier's part code. Every value MUST be kept as it is in the
file, including values that look like numbers, such as an MPN of "`1234`" or a package of "`0402`" (`ISSUE-23`). A file
with only the headers has no parts. When the parts are loaded, the core MUST log
"`stores_app: Loaded parts. source: '<source>', part_count: <count>, revision: '<revision>'`" at info level.

**STORE-2 — Viewing parts.** *Confirmed.* The core MUST return the parts loaded from a data source when asked
(`RequestPartsView`), in the order the data source has them, with each part's other values. *Observed*: the view MUST
name the other values in the order of their names, and the core MUST refuse to return the parts of a data source that
isn't loaded. A data source can be asked for as it was opened, or as the stores service identified it. The core's view
model MUST say, for each data source it was asked for, how the stores service identified it, whether its parts are
loaded, whether they're being read, and why the last load was refused, until one succeeds.

**STORE-3 — Refusing a data source.** *Observed.* The core MUST refuse to load a data source that can't be read, such as
a file that doesn't exist, or that lacks a header, and MUST say why, naming the header that's missing, for example
"`Missing header. header: 'Manufacturer'`". The parts loaded from it before, if any, MUST be kept. The refusal is the data
source's, so every front end and view using it can show it.

**STORE-4 — Listing parts from the CLI.** *Observed.* `planner_cli parts --source <file> list` MUST open the data source
(`STORE-5`) and write its parts to standard output as CSV, every value quoted, with the headers "`Manufacturer`", "`Mpn`",
and then the other values' names, in the order of the view (`STORE-2`). If the core refuses, the CLI MUST fail with the
core's error.

**STORE-5 — Opening a data source once.** *Confirmed.* A data source used by more than one view, for example by two tabs,
MUST be loaded once, and its parts kept once by the core. Opening a data source (`OpenParts`) MUST load its parts, unless
they're loaded already, and then send its parts view (`STORE-2`). Only loading it again (`LoadParts`, `STORE-1`) MUST
read it again. *Observed*: when the parts are loaded already, the core MUST log
"`stores_app: Parts already loaded. source: '<source>'`" at info level. A data source opened as it was before isn't
identified again. A data source opened while it's being read for another open isn't read again, and is shown when the
first read is done, as every view of a data source is. A refused open MUST send no view, and opening the data source again
MUST try to load it again.

**STORE-6 — One file, one data source.** *Confirmed.* A file MUST be one data source however its path is written, for
example with "`..`", or, on Windows, in a different case. *Observed*: the stores service MUST identify a file by its
canonical path, without Windows' "`\\?\`" prefix where it isn't needed, and the core MUST use the data source as the
service identified it in its logs and errors, and in the views it sends, whose data source may be written differently
from the event's. A data source whose file doesn't exist MUST be kept as it is, so that the error names it as the user
did.

**STORE-7 — The stores service.** *Confirmed.* The cores MUST NOT read or write a data source themselves. A core MUST ask
its shell for each store operation, as a Crux effect, and be given the result as an event, so that it's tested with
results made up by its tests, with no files. The operations MUST be done by the stores service, a library every shell
uses, so that no shell does them itself, and each process MUST have one, used by all of its cores. The service MUST be
async, so that a shell without threads, such as a web shell, can use it, with back ends of its own. *Observed*: the
operations and their results are in `planning/stores_protocol`, which has no IO, so a core using them stays pure. Each
back end is a module of the service, chosen by the data source: `csv_file` for files today. The service MUST log each
operation it's asked to do, as "`stores_service: Performing stores operation. operation: <operation>`" at debug level.
An operation's result says how many parts were read, not what they are, as it's logged. A core given the result of a
different operation MUST refuse it, as a fault in the shell. The planner's core uses the stores service to write a
project's file, when it's created and when it's saved (`STORE-10`). *Target*: the planner's core and the Variant
Builder's core MUST use the stores service for every file they read and write. The gap: they read and write the others
themselves, so they share nothing with the stores core, and nothing tells them when a file changes. They're being moved
to the stores service an event at a time (`../../missions/variant-builder.md`).

**STORE-8 — Revisions.** *Confirmed.* Each view of a data source MUST say which revision of the data source it shows, so
that a view that's out of date can be known, and a change can say which revision it was based on. Two reads of a data
source MUST have the same revision only if its contents were the same, in any process. *Observed*: the revision is made
by the back end, and only compared; a file's is the SHA-256 hash of its contents, read once, with the parts. The core's
view model MUST say each data source's revision. Loading a data source whose revision hasn't changed MUST NOT send a
view, as every view of it shows that revision already, and the core MUST log
"`stores_app: Parts unchanged. source: '<source>', revision: '<revision>'`" at info level. It still clears the error of
a load refused before.

**STORE-9 — Watching.** *Confirmed.* A data source MUST be watched once it's identified, once however often, and by
however many tabs, it's opened, so that a change made by anyone, such as the CLI, an editor or another application, is
shown by every view of it, without the user asking. A change MUST read the data source again, as loading it would, so an
unchanged revision sends no view (`STORE-8`), and a refused read says why and keeps the parts loaded before (`STORE-3`).
A change while the data source is being read MUST read it again once that read is done, and the parts of the first read
MUST NOT be sent, as they may be out of date. The service's own reads MUST NOT be changes, or a data source would be
read again and again. *Observed*: a file is watched by watching its folder, so that a file replaced by another, as an
editor may save it, is still watched, and only changes to the file's contents, or its being created or removed, are
changes. A watch is an operation whose results are a stream, one for each change, until the shell drops it. The service
MUST log "`stores_service::csv_file: Watching the data source. source: '<source>'`" at info level, or
"`Unable to watch the data source.`" at warn level, saying why, in which case the data source is still shown, though a
change isn't. The core MUST log "`stores_app: Data source changed. source: '<source>'`" at info level for each change,
and "`stores_app: Data source changed while being read, reading it again. source: '<source>'`" when it reads it again.
The GUI watches each data source it shows. The CLI doesn't, as it's done before a data source could change.

**STORE-10 — Documents.** *Confirmed.* A document, such as a project, a PCB or a process, MUST be read and written by the
stores service, as it is: the core that keeps it makes its contents, and reads them, so that the service needn't know
what's in it, and the core is tested with documents kept in memory by its tests. Reading a document that doesn't exist
MUST say so, and not be an error, so that a core can tell it apart from one that can't be read. A refused write MUST
say why, naming the path. Each read and write MUST say the document's revision (`STORE-8`). A change MUST only be said
to be saved once its document is written, and only if the document hasn't changed again since it was sent to be
written, so that a later change isn't lost, and a refused write MUST keep the changes unsaved. *Observed*: the back end
is `document_file`, which writes the contents given, so a project's file is as it was when the core wrote it itself. The
operations are logged as the service's others are (`STORE-7`), saying how long the contents are, not what they are, as
a document may be large. The planner's core logs "`planner_app: Received event. event: ProjectWritten {`" when it's
given the result, then "`Created project successfully.`" or "`Saved project. path: <path>`" at info level. Loading a
project MUST read it, then give each of its processes that has no file a file of its own, written by the service, then
read each PCB it uses that isn't loaded already, and only then load it, so a project that can't be read isn't loaded.
A PCB that can't be read says why, and the project is still loaded, with the PCBs read before it. The core logs
"`planner_app: Load project. path: <path>`", then "`planner_app: Loading PCB. path: <path>`" for each PCB, at info
level, and "`planner_app: Received event. event: ProjectLoaded {`", which says which PCBs were read, not what's in them.
A PCB MUST be read and written by the service too: when it's created, loaded, saved, refreshed or added to a project.
A PCB is only created, or added to a project, once it's written, or read. A project's PCBs are read when it's loaded,
or when they're added, so an event that needs one that couldn't be read MUST say "`PCB not loaded.`", and not read it
itself. The core logs "`planner_app: Saving PCB. path: <path>`" as it asks for a PCB to be written, and
"`planner_app: Saved PCB. path: <path>`" once it is, and "`planner_app: Received event. event: PcbWritten {`",
"`PcbRead {`" or "`ProjectPcbRead {`" when it's given the result. A process document MUST be read and written by the
service too: when it's created, loaded, saved, added to a project, or when the project's processes are refreshed, for
which every file is read before any process is changed, so nothing changes if one can't be read. A process is only
created, or added, once it's written, or read. The core logs "`planner_app: Saving process. path: <path>`" and
"`planner_app: Saved process. path: <path>`", and "`planner_app: Loading process. path: <path>`", at info level.

## Acceptance criteria

- `planning/stores_app/tests/parts.rs`, with no files, answers the operations the core asks for itself, and checks the
  core asks to identify the data source and then read it, and sends the view, with the parts in order, the other values'
  names in the order of their names, and the data source as identified, and that the view can be asked for by the data
  source as opened or as identified. It checks the view model's status of the data source, a data source with no parts,
  and that loading again reads it again and sends the changed parts (`STORE-1`, `STORE-2`, `STORE-7`).
- `planning/stores_app/tests/parts.rs` checks a refused read says why, sends no view and keeps the parts loaded before,
  that a refused open sends no view and that opening again reads the data source again, and that parts that aren't
  loaded can't be viewed (`STORE-2`, `STORE-3`, `STORE-5`).
- `planning/stores_app/tests/parts.rs` checks that opening again asks for nothing, that a data source written
  differently is identified, and not read again, and that two opens before the parts are read read them once
  (`STORE-5`, `STORE-6`).
- `planning/stores_app/tests/parts.rs` checks the result of a different operation is refused (`STORE-7`).
- `planning/stores_app/tests/parts.rs` checks the view and the view model say the revision read, that loading again an
  unchanged data source sends no view, and that a changed one sends its new revision, and that an unchanged load after
  a refused one clears the error (`STORE-3`, `STORE-8`).
- `planning/stores_app/tests/parts.rs` checks a data source is watched once it's identified, and once however often it's
  opened, that a change reads it again and sends its view, that a change leaving its contents as they were sends no
  view, that a change leaving it unreadable says why and keeps the parts, and that a change while it's read reads it
  again, sending only the parts read after the change (`STORE-3`, `STORE-8`, `STORE-9`).
- `planning/stores_service/tests/watch.rs` checks a watch says when the file is written, and when it's replaced by
  another renamed over it, and says nothing when it's read, or when another file in its folder is written (`STORE-9`).
- `planning/stores_service/tests/document_file.rs` writes a document and reads it as it was written, with the revision
  written, checks writing again replaces it, with a revision that changes only if its contents do, that a document that
  doesn't exist is none, and that one that can't be written says why, naming its path (`STORE-8`, `STORE-10`).
- `planning/planner_app/tests/documents.rs`, with no files, answers the planner's core's writes itself, and checks
  creating a project writes its file, ending with a new line, with no unsaved changes, that a project whose file can't
  be written isn't created, says why, and keeps the project before it, that saving writes the project's changes, that a
  refused save keeps them unsaved, that a change made while the project is written is still unsaved once it is, and that
  the result of a different operation is refused (`DOC-1`, `STORE-7`, `STORE-10`).
- `planning/planner_app/tests/documents.rs` checks loading reads the project and then each of its PCBs, with no unsaved
  changes, and that loading again doesn't read the PCBs again. It checks a project that doesn't exist, can't be read, or
  isn't a project, says why and isn't loaded, and that a PCB that can't be read says why, stops the PCBs being read, and
  leaves the project loaded. It checks a process without a file is given one, written by the service, with the project
  then having unsaved changes, that a file with the same process is used, and that one with another is kept, the process
  being given another file (`STORE-10`).
- `planning/planner_app/tests/documents.rs` checks creating a PCB writes its file, with no unsaved changes, that one
  whose file can't be written isn't created, that saving one writes it, and that a refused save keeps its changes
  unsaved, that saving every PCB writes each, that loading one reads it, and that one that doesn't exist says so, that
  refreshing reads each again, that adding one to a project reads it, and that one that can't be read isn't added, and
  that an event needing a project's PCB that couldn't be read says so, without reading it (`DOC-1`, `STORE-10`).
- `planning/planner_app/tests/documents.rs` checks creating a process writes its file, with no unsaved changes, and
  that one whose file can't be written isn't created, that saving one writes it, that a refused save keeps its changes
  unsaved, and that saving every process writes each, that loading one reads it, and that one that doesn't exist says
  so, that adding one to a project reads it, and that one that can't be read isn't added, and that refreshing reads
  each of the project's processes, changing nothing if one can't be read (`DOC-1`, `STORE-10`).
- `planning/planner_cli/tests/planner.rs` checks "`planner_app: Load project. path: `", then
  "`planner_app: Loading PCB. path: `" and "`panel_a.pcb.json`", are logged, in order, as a project with a PCB is
  loaded, and its other tests check the processes are given files as before (`STORE-10`).
- `tests/ui/open_project.rs`, in the GUI, opens a project, and checks "`planner_app: Received event. event: Load {`",
  "`stores_service: Performing stores operation. operation: ReadDocument { path: `" and
  "`planner_app: Received event. event: ProjectLoaded {`" are logged, in order, with no "`Event failed.`"
  (`STORE-10`).
- `planning/planner_cli/tests/planner.rs` creates a project, and checks
  "`planner_app: Received event. event: CreateProject {`", "`Creating project. path: `",
  "`stores_service: Performing stores operation. operation: WriteDocument { path: `",
  "`planner_app: Received event. event: ProjectWritten {`", "`output: DocumentWritten(Ok(`" and
  "`Created project successfully`" are logged, in order, and that the file is as before (`DOC-1`, `STORE-10`).
- `planning/stores_service/tests/csv_file.rs` checks a file's revision is the same while its contents are, however its
  path is written, and when it's written again unchanged, that it changes when they do, and that another file with the
  same contents has the same revision (`STORE-8`).
- `planning/stores_service/tests/csv_file.rs` reads a CSV file with other values, a file with only headers, and keeps
  values that look like numbers, says which header is missing, and that a file that doesn't exist can't be read
  (`STORE-1`, `STORE-3`, `ISSUE-23`). It checks a file is identified by its canonical path, without the "`\\?\`"
  prefix, however its path is written, on Windows in a different case too, and that a data source whose file doesn't
  exist is kept as it is (`STORE-6`).
- `planning/planner_cli/tests/parts.rs` lists a file's parts, checks the output, and checks
  "`stores_app: Received event. event: OpenParts {`",
  "`stores_service: Performing stores operation. operation: ResolveSource {`",
  "`stores_app: Received event. event: SourceResolved {`",
  "`stores_service: Performing stores operation. operation: ReadParts {`",
  "`stores_app: Received event. event: PartsRead {`", "`stores_app: Loaded parts. source: '`" and "`part_count: 2`" are
  logged, in order, with no "`Event failed.`", and no "`Watching the data source.`" (`STORE-4`, `STORE-7`, `STORE-9`).
- `planning/planner_cli/tests/parts.rs` lists a file without the "`Manufacturer`" header, checks the CLI fails saying
  which header is missing, and that "`stores_app: Received event. event: OpenParts {`",
  "`stores_service: Performing stores operation. operation: ReadParts {`",
  "`stores_app: Received event. event: PartsRead {`" and "`stores_app: Event failed. error: UnableToLoadParts {`" are
  logged, in order (`STORE-3`, `STORE-4`).
- `tests/ui/parts_sources.rs`, in the GUI, checks "`stores_app: Parts already loaded. source: '`" is logged, and no
  "`Loaded parts.`", when a second tab opens a data source another tab shows (`STORE-5`, `STORE-6`, and `PARTSRC-5` in
  `../gui/12-parts-sources.md`), and that the stores service's operations and their results are logged, in order
  (`STORE-7`). It checks a change to the file, made outside the GUI, is shown by both tabs, with
  "`stores_app: Received event. event: SourceChanged {`", "`stores_app: Data source changed. source: '`" and
  "`stores_app: Loaded parts. source: '`" logged, in order, with no "`ReloadClicked`" and no "`event: LoadParts`", and
  that the second tab starts no second watch (`STORE-9`).
