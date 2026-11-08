# Mission: the Variant Builder in the Planner

The Planner's GUI helps the user turn their EDA tool's output into an assembly variant and a BOM, using the
Variant Builder's core, instead of the Variant Builder's CLI. These are the mission's first, high-level requirements, to
come back to as the work goes on. Each step is specified, in `../specs/`, as it's built.

## The problem

Building a variant with the Variant Builder's CLI (`variantbuilder_cli build`) goes like this:

1. The user runs the CLI, adding arguments one at a time until it has all it needs. Most point at CSV files that the
   core loads parts, part mappings, substitutions, assembly rules, the load-out and so on from.
2. When the tool eventually runs, it usually complains that a CSV file's headers are missing or wrong. The user makes the
   files, with the right headers and no rows, to satisfy it.
3. The user runs it again, and is shown the placements, often hundreds of lines, with an error next to each.
4. The user then repeats a cycle: update the parts, mappings, substitutions, rules and so on, and run the tool again,
   until there are no errors. They may also need more arguments, for example to exclude placements by ref-des.

The cycle takes a long time, especially for a design with many components the user hasn't used with the Variant Builder
before. It gets better over time, as the parts files come to hold more of the parts the user uses.

It also takes a long time because the user has to search and cross-reference their inventory system, or their purchase
receipts, to find out exactly what they bought and whether they still have any, and to build a shopping list of the
components they want to use, before they can enter each manufacturer and MPN and make mapping rules.

For even a mildly complicated board, this can take a day.

## The goal

A workflow built around the user, which shows them exactly what they need to do next, and helps them build their
shopping list and BOM, a step at a time, and mark BOM items as confirmed.

- **Tables that are linked.** The placements the EDA files specify, the shopping list and the BOM items are each shown
  in a table. Clicking a row in one selects the corresponding rows in the others.
- **The PCB follows.** Clicking a placement focuses the corresponding PCB on it, if a PCB is assigned and open.
- **An explorer.** A variant builder has an explorer tree, from which further tabs are shown, for example its parts,
  its substitutions, and the results of a build.
- **Data sources are top-level.** The parts, part mappings, substitutions, rules and so on, currently CSV files, are
  shared by many variant builders, so they are top-level objects of their own, as projects, PCBs and processes are. For
  example, one parts data source can be used by two variant builders at once.
- **Data sources without a variant builder.** The user can view and change their parts, substitutions and other data
  sources from the UI, even when no project or variant builder is open.
- **Changes are seen everywhere.** A change committed to a data source is reflected in every open variant builder that
  uses it. A manual refresh or build is acceptable, updating automatically is preferred.
- **Data sources may change.** Data sources are CSV files today, and may later be databases or other back ends, so the
  front ends use them through a core, not by reading the files themselves.

## How it's built

- **Iteratively.** A step at a time, each committed on its own, and each usable on its own where it can be. A core comes
  before the front ends that use it.
- **Core first.** The work is done by cores the CLI can use too (`../specs/planner/README.md`, "Principles"). The data
  sources have a core of their own, the stores core (`planning/stores_app`), shared by the front ends.
- **Tested.** Every step has its core's and CLI's tests, and every step that changes the GUI has GUI tests, driven as a
  user would drive the GUI.

## Architecture

Decided on 2026-11-04, when it was found that the planner's core and the stores core would each keep their own copy of
a data source, with nothing to keep them in step, which matters once data sources can be changed.

- **The cores are pure.** If Crux is used, it's used throughout: a core never reads or writes a file or a database. It
  asks its shell for each store operation as a Crux effect, and is given the result as an event, so it's tested quickly,
  with results made up by its tests.
- **One stores service, for every shell.** The store operations are done by `planning/stores_service`, a library both
  Rust shells, the CLI and the GUI, use, so they aren't done twice. Each process has one, which every core in it uses,
  so a project and a parts tab use the same data source. The operations' types are in `planning/stores_protocol`.
- **Async.** The service isn't tied to a thread, so a web shell, built for WebAssembly, can use it too, with back ends of
  its own. Each back end is a module, chosen by the data source: a CSV file on a native file system today, later a
  database, a stores server over HTTP, or the browser's storage.
- **Views may go out of date, changes are checked.** A view of a data source carries the data source's revision. A
  change says which revision it was based on, and is refused if the data source has changed since, as happens when
  another process, or another connection to a database, changed it. A refusal, or a notice that a data source changed,
  makes every core and view using it ask again.
- **Changes from other processes are noticed.** Each core asks the service to watch the data sources it uses, and is
  told when one changes, whoever changed it, for example the CLI, or an editor, and reads it again (`STORE-9`).
- **Changes name columns and rows, not places.** Decided on 2026-11-07, as a data source may later be a database, whose
  queries may return some of a table's columns, or columns from many tables, or a web API. A table's view has each
  row's key, made by the back end, such as a database's primary key, or, for a CSV file, the row's place when it was
  read, and each column's reference, as the data source names it, such as a CSV file's header, or a query result's
  field. A change says which row, by its key, and which columns, by their references, with each value as it was and as
  it's to be, and the back end changes only those, refusing the change if a value isn't as it was, as an SQL
  `UPDATE ... WHERE COL_A = OLD_VALUE_A` would. So a column the table doesn't show is never touched, and a change to
  one row doesn't refuse a change to another. The revision says whether a view is out of date, and isn't checked by a
  change. A table says how its rows are ordered: not at all, so they can't be moved, by their place, as a CSV file's
  lines are, or by a column's values, which a database or a web API would have. A row is moved to before another, or
  to the end, named by their keys, so a front end showing some of the rows, such as a page of them, can move one, and
  the back end works out the ordering values.
- **No shared process for now.** Each process has its own service; the file or database is the record of what's
  stored, and the revision checks keep changes safe across processes. A stores server, shared by many processes, may
  come later, using the same service.

## Steps

| Step | What | Status |
|---|---|---|
| 1 | The stores core (`planning/stores_app`): load a parts data source, a CSV file for now, and view its parts. | Done |
| 2 | The CLI uses the stores core: `planner_cli parts --source <file> list`. | Done |
| 3 | "Open parts" in the GUI: choose a parts data source, and show its parts in a table, through the stores core. A data source shown by many tabs is loaded once, and kept once, and reloading it in one tab updates them all. | Done |
| 4 | The stores service (`planning/stores_service`), async, with a CSV file back end. The stores core is pure, and the CLI and the GUI have the service do its operations. | Done |
| 5 | Revisions: each view of a data source says which revision of it it shows. | Done |
| 6 | Watching: a change to a data source's file, by anyone, is noticed, and every core and view using it is updated. | Done |
| 7 | The planner's core and the Variant Builder's core are pure, and use the stores service for every file they read and write, an event at a time. | Done |
| 8 | Changing parts, checked against the revision the change is based on, from the core, the CLI and the GUI. | Done |
| 9 | A variant builder: define the variant and choose its data sources. | Done |
| 10 | A variant builder's explorer, and its tabs for its data sources and its build's results. | Done, but 10e |
| 11 | Changing a data source's contents from the UI, seen by every variant builder using it. | Started |
| 12 | Placements, shopping list and BOM tables, linked to each other and to the PCB. | |
| 13 | Confirming BOM items. | |

### Step 7, an event at a time

Each is a commit. A document, a project, a PCB or a process, is read and written as it is, the core making and reading
its contents (`STORE-10`). A data source, such as a load-out or packages, is read and written by the service, which knows
its format, as parts are. The GUI's planner cores wait for the service for now, as they waited for the files before, so
what they show is as it was.

| Events | What | Status |
|---|---|---|
| `CreateProject`, `Save` | The project's file is written by the service, and a project is only created once it is. | Done |
| `Load` | The project, its PCBs, and moving its processes to their own files, read and written by the service, in one command. | Done |
| `CreatePcb`, `CreateProjectPcb`, `LoadPcb`, `SavePcb`, `SaveAllPcbs`, `AddPcb`, `RefreshPcbs` | PCB documents. A PCB is only created, or added, once it's written, or read, and an event needing a PCB that couldn't be read says so. | Done |
| `CreateProcess`, `LoadProcess`, `SaveProcess`, `SaveAllProcesses`, `AddProcess`, `RefreshProcesses` | Process documents. A process is only created, or added, once it's written, or read. | Done |
| Load-outs, packages, package mappings, placements | The project's data sources, used by many events, read before the event, which is processed again with them, and changes nothing unless it succeeds (`STORE-11`). | Done |
| `GenerateArtifacts` | The files made for a phase, and the report, made by the core and written by the service, in order (`STORE-12`). | Done |
| `RecordPhaseOperation` | Each phase's operation history, read and written by the service as a document (`STORE-13`). | Done |
| The Variant Builder's core | EDA placements, parts, part mappings, substitutions, assembly rules and load-outs read by the service as records, and the placements and BOM written as documents (`STORE-14`). | Done |

### Step 8, changing parts

Each is a commit, the core first.

| What | Status |
|---|---|
| The stores core: add a part, set its other values, or remove it, based on a revision, and refused if the data source changed since, by the core or the stores service (`STORE-15`). | Done |
| The CLI: `planner_cli parts --source <file> add`, `set` and `remove`, based on the revision read, or one given (`STORE-16`). | Done |
| The GUI: add and remove parts from a parts tab, seen by every tab showing the data source (`PARTSRC-6`). Setting a part's other values from the GUI is left to step 11. | Done |

### Step 9, a variant builder

A variant builder is a file, `<name>.variant.json`, a top-level document of its own, like a PCB, which defines the
variant, its data sources and its outputs (`../specs/variantbuilder/`), so the user defines it once, changes it a
little at a time, and builds it again. The CLI and the GUI create it, change it and build it with the same terms:
create, configure and build. Each is a commit, the core first.

| What | Status |
|---|---|
| 9a. The Variant Builder's core creates, opens, configures and builds a variant builder, through the stores service (`VARB-1` to `VARB-6`). | Done |
| 9b. The CLI: `variantbuilder_cli create --file <file>`, refusing to replace a file without `--overwrite`, `configure --file <file>`, with the arguments `build` has today, each optional, setting only those given, and `build --file <file>`. `build`'s arguments move to `configure`, and the core's `BuildWith` event, which builds without a file, is removed (`VARBCLI-1` to `VARBCLI-3`). | Done |
| 9c. The GUI: "New variant builder" and "Open variant builder" open a tab in which the variant is defined and each data source chosen, and "Build" builds it. The explorer, and the tabs for its data sources and results, are step 10 (`VBGUI`). | Done |
| 9d. A design variant: in the GUI, "New design variant" and "Open design variant"; a new one starts with its EDA tool and placements, and its design and variant names, which name its file and are kept as they are; the rest is shown in groups, with global and design substitutions, reordered with up and down buttons and shown in the order they're applied, and new assembly rules. The core and the CLI's options are renamed to match (`VARB-1`, `VARB-2`, `VARB-8`, `VARBCLI-1`, `VARBCLI-2`, `VARBCLI-4`, `VBGUI-1` to `VBGUI-3`, `VBGUI-6`, `VBGUI-7`). | Done |
| 9e. A list of load-outs, one for each pick-and-place machine of a job, the variant preferring the parts already in any of them, shown in the GUI as a list of files (`VARB-1`, `VBGUI-2`, both *Target*). | Later |

### Step 10, a variant builder's explorer

A design variant's tab has panels, as a project's has: an explorer, whose tree has a node for each of the variant
builder's data sources, in folders named after the "Overview" panel's groups, and its build's outputs, and the
"Overview" panel, the form of step 9. Double-clicking a node opens its panel, or shows it if it's open. Each data
source and output is shown by the stores core, read-only, as a table, so the panels are improved over time, and the
data sources changed from them in step 11. Each is a commit, the core first.

| What | Status |
|---|---|
| 10a. The stores core shows a data source's records as a table, read-only and watched, as it shows parts (`STORE-17`). | Done |
| 10b. The CLI lists a data source's table: `planner_cli table --kind <kind> --source <file> list` (`STORE-18`). | Done |
| 10c. The Variant Builder's core says where its variant's outputs are written, so a front end can show them (`VARB-9`). | Done |
| 10d. The GUI: the design variant's explorer, its "Overview" panel, a panel for each data source, a "Build output" panel, a placeholder for the mapping results to come, and the variant's placements and BOM (`VBGUI-9` to `VBGUI-12`). | Done |
| 10e. The "Build output" panel shows the mapping results. To be designed with the project's owner. | Later |

### Step 11, changing a data source from the UI

Decided with the project's owner on 2026-11-07: a data source's table is changed in place, a value edited in its cell,
and rows added, removed and moved, each change written at once, checked against the values shown, so nothing is left
unsaved, and no one's change is lost, and every view of the data source, in every variant builder, shows it. Part
mappings first, as they're what the build's cycle changes most, then substitutions, assembly rules, and parts' other
values, left over from step 8. A design's EDA placements, and a variant's placements and BOM, stay read-only, as other
tools write them. Each is a commit, the core first.

| What | Status |
|---|---|
| 11a. The stores core changes a table's rows, updating, adding, removing and moving them, by their keys and their columns' references, each checked against the values the change was based on, part mappings first, and the stores service changes only those (`STORE-19`). | Done |
| 11b. The CLI changes part mappings: `planner_cli table --kind part-mappings --source <file> add`, `set`, `remove` and `move`, each naming a row by its key, as `list --keys` lists it (`STORE-20`). | Done |
| 11c. The GUI: a part mappings panel's values are edited in their cells, a pattern shown as what it is as it's typed, and its rows added, removed and moved (`VBGUI-13`), a row selected by clicking a cell, and the selection following a row button's change, with changes to `egui_deferred_table`. | Done |
| 11c.1. The domain: the EDA tools' fields are the domain's, shared by the cores, the stores service and the readers, and a part mapping has a pattern for each EDA tool's field the data source has, whichever tool it has (`STORE-17`). | Done |
| 11c.2. The stores core and service add a table's columns, named, with where they go, or those an EDA tool needs, as the core works them out (`STORE-21`). | Done |
| 11c.3. The CLI adds a table's columns: `planner_cli table --kind part-mappings --source <file> add-columns`, with `--eda <TOOL>`, or `--column <NAME>` and where they go (`STORE-20`). | Done |
| 11c.4. The GUI: choosing a row's EDA tool, or adding a row, adds the tool's columns first, if they're needed, where the data source puts them (`STORE-21`), each pattern's, manufacturer's and MPN's cell is coloured by whether it's as the row needs it, and each row has a status, saying its problems (`VBGUI-13`). | Done |
| 11d. Substitutions, as part mappings are: the core and the service, the CLI, then the GUI. | Done |
| 11d.1. The stores core and service: a substitution has a pattern and a value for each EDA tool's field the data source has, its rows are changed, and an EDA tool's columns added, each group's after its last (`STORE-17`, `STORE-19`, `STORE-21`). | Done |
| 11d.2. The CLI changes substitutions: `planner_cli table --kind substitutions --source <file>` `add`, `set`, `remove`, `move` and `add-columns`, with `--value <FIELD>=<VALUE>` and `--comment` (`STORE-20`). | Done |
| 11d.3. The GUI: a substitutions' panel changes them as a part mappings' does, its value cells coloured, and its rows' statuses saying their problems (`VBGUI-14`). | Done |
| 11e. Assembly rules, as part mappings are. | |
| 11f. A parts tab's other values, edited in their cells (`PARTSRC`). | |
