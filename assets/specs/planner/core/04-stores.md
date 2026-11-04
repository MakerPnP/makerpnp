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
source's current parts. *Observed*: a parts CSV file MUST have the headers "`Manufacturer`" and "`Mpn`", and its other
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

## Acceptance criteria

- `planning/stores_app/tests/parts.rs` loads a CSV file with other values and checks the view's parts, their order and the
  other values' names, loads a file with only headers, loads a changed file again, and keeps values that look like
  numbers (`STORE-1`, `STORE-2`, `ISSUE-23`).
- `planning/stores_app/tests/parts.rs` checks a file without the "`Manufacturer`" header, and a file that doesn't exist,
  are refused, saying why, and that parts that aren't loaded can't be viewed (`STORE-2`, `STORE-3`).
- `planning/stores_app/tests/parts.rs` checks a refused load keeps the parts loaded before (`STORE-3`).
- Not covered yet: the log lines, "`stores_app: Received event.`", "`stores_app: Event failed.`" and
  "`stores_app: Loaded parts.`", which the CLI's tests will check in order once the CLI uses the stores core (`LOG-1`,
  `LOG-2`, `STORE-1`).
