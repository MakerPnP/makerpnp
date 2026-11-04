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
different operation MUST refuse it, as a fault in the shell. *Target*: the planner's core and the Variant Builder's core
MUST use the stores service too. The gap: they read and write their files themselves, so they share nothing with the
stores core, and nothing tells them when a file changes.

**STORE-8 — Revisions.** *Confirmed.* Each view of a data source MUST say which revision of the data source it shows, so
that a view that's out of date can be known, and a change can say which revision it was based on. Two reads of a data
source MUST have the same revision only if its contents were the same, in any process. *Observed*: the revision is made
by the back end, and only compared; a file's is the SHA-256 hash of its contents, read once, with the parts. The core's
view model MUST say each data source's revision. Loading a data source whose revision hasn't changed MUST NOT send a
view, as every view of it shows that revision already, and the core MUST log
"`stores_app: Parts unchanged. source: '<source>', revision: '<revision>'`" at info level. It still clears the error of
a load refused before.

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
  logged, in order, with no "`Event failed.`" (`STORE-4`, `STORE-7`).
- `planning/planner_cli/tests/parts.rs` lists a file without the "`Manufacturer`" header, checks the CLI fails saying
  which header is missing, and that "`stores_app: Received event. event: OpenParts {`",
  "`stores_service: Performing stores operation. operation: ReadParts {`",
  "`stores_app: Received event. event: PartsRead {`" and "`stores_app: Event failed. error: UnableToLoadParts {`" are
  logged, in order (`STORE-3`, `STORE-4`).
- `tests/ui/parts_sources.rs`, in the GUI, checks "`stores_app: Parts already loaded. source: '`" is logged, and no
  "`Loaded parts.`", when a second tab opens a data source another tab shows (`STORE-5`, `STORE-6`, and `PARTSRC-5` in
  `../gui/12-parts-sources.md`), and that the stores service's operations and their results are logged, in order
  (`STORE-7`).
