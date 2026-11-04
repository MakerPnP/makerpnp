# Core 04 — Stores

The data sources the user's work draws on, such as their parts, and the core the front ends use them through, the
stores core (`planning/stores_app`). It is a core of its own, beside the planner's, because a data source is shared:
by many variant builders, by more than one window of a front end, and by every front end
(`../../../missions/variant-builder.md`).

A data source is a CSV file today. It may later be a database or another back end, so the front ends use a data source
only through the stores core, never by reading its files.

The stores core logs each event it receives, and why it refused one, as the planner's core does (`LOG-1`, `LOG-2`), its
target being `stores_app`, for example "`stores_app: Received event. event: LoadParts {`".

These requirements were decided by the project's owner, those marked *Observed* are details of how they were done, and
have not been reviewed.

**STORE-1 — Loading parts.** *Confirmed.* The core MUST load the parts of a parts data source when asked
(`LoadParts`). Loading a data source again MUST replace the parts loaded from it before, so that it shows the data
source's current parts. *Observed*: once loaded, the core MUST send the data source's parts view (`STORE-2`), so that a
front end can update every view of the data source. A parts CSV file MUST have the headers "`Manufacturer`" and "`Mpn`", and its other
headers are the parts' other values, for example a supplier's part code. Every value MUST be kept as it is in the file,
including values that look like numbers, such as an MPN of "`1234`" or a package of "`0402`" (`ISSUE-23`). A file with
only the headers has no parts. When the parts are loaded, the core MUST log "`stores_app: Loaded parts. source:
'<source>', part_count: <count>`" at info level.

**STORE-2 — Viewing parts.** *Confirmed.* The core MUST return the parts loaded from a data source when asked
(`RequestPartsView`), in the order the data source has them, with each part's other values. *Observed*: the view MUST
name the other values in the order of their names, and the core MUST refuse to return the parts of a data source that
isn't loaded.

**STORE-3 — Refusing a data source.** *Observed.* The core MUST refuse to load a data source it can't read, such as a
file that doesn't exist, or that lacks a header, and MUST say why, naming the header that's missing, for example
"`Missing header. header: 'Manufacturer'`". The parts loaded from it before, if any, MUST be kept.

**STORE-4 — Listing parts from the CLI.** *Observed.* `planner_cli parts --source <file> list` MUST open the data source
(`STORE-5`) and write its parts to standard output as CSV, every value quoted, with the headers "`Manufacturer`", "`Mpn`", and
then the other values' names, in the order of the view (`STORE-2`). If the core refuses, the CLI MUST fail with the core's
error.

**STORE-5 — Opening a data source once.** *Confirmed.* A data source used by more than one view, for example by two tabs,
MUST be loaded once, and its parts kept once by the core. Opening a data source (`OpenParts`) MUST load its parts, unless
they're loaded already, and then send its parts view (`STORE-2`). Only loading it again (`LoadParts`, `STORE-1`) MUST
read it again. *Observed*: when the parts are loaded already, the core MUST log
"`stores_app: Parts already loaded. source: '<source>'`" at info level. A refused open MUST send no view, and opening the
data source again MUST try to load it again.

**STORE-6 — One file, one data source.** *Confirmed.* A file MUST be one data source however its path is written, for
example with "`..`", or, on Windows, in a different case. *Observed*: the core MUST use the file's canonical path, without
Windows' "`\\?\`" prefix where it isn't needed, for each event's data source, in its logs and errors, and in the views it
sends, whose data source may be written differently from the event's. A data source whose file doesn't exist MUST be kept
as it is, so that the error names it as the user did.

## Acceptance criteria

- `planning/stores_app/tests/parts.rs` loads a CSV file with other values and checks the view sent by loading it, and by
  asking for it, the view's parts, their order and the other values' names, loads a file with only headers, loads a
  changed file again, and keeps values that look like numbers (`STORE-1`, `STORE-2`, `ISSUE-23`).
- `planning/stores_app/tests/parts.rs` checks a file without the "`Manufacturer`" header, and a file that doesn't exist,
  are refused, saying why, and that parts that aren't loaded can't be viewed (`STORE-2`, `STORE-3`).
- `planning/planner_cli/tests/parts.rs` lists a file's parts, checks the output, and checks
  "`stores_app: Received event. event: OpenParts {`", "`stores_app: Loaded parts. source: '`" and "`part_count: 2`" are
  logged, in order, with no "`Event failed.`" (`STORE-4`).
- `planning/planner_cli/tests/parts.rs` lists a file without the "`Manufacturer`" header, checks the CLI fails saying
  which header is missing, and that "`stores_app: Received event. event: OpenParts {`" and
  "`stores_app: Event failed. error: UnableToLoadParts {`" are logged, in order (`STORE-3`, `STORE-4`).
- `planning/stores_app/tests/parts.rs` checks a refused load keeps the parts loaded before (`STORE-3`).
- `planning/stores_app/tests/parts.rs` opens a file twice, changing it in between, and checks the second open sends the
  parts loaded by the first, and that loading it again sends the changed parts. It checks a refused open sends no view,
  and that opening again, once the file is fixed, loads it (`STORE-3`, `STORE-5`).
- `planning/stores_app/tests/parts.rs` opens a file, then opens it with its path written with "`..`", and checks the
  parts aren't loaded again and that the view's data source is the canonical one. On Windows, it checks a path in a
  different case is the same data source. It checks a data source whose file doesn't exist is kept as it is
  (`STORE-6`).
- `tests/ui/parts_sources.rs`, in the GUI, checks "`stores_app: Parts already loaded. source: '`" is logged, and no
  "`Loaded parts.`", when a second tab opens a data source another tab shows (`STORE-5`, `STORE-6`, and `PARTSRC-5` in
  `../gui/12-parts-sources.md`).
