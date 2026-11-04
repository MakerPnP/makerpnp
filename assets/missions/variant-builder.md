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
| 7 | The planner's core and the Variant Builder's core are pure, and use the stores service for every file they read and write, an event at a time. | Next |
| 8 | Changing parts, checked against the revision the change is based on, from the core, the CLI and the GUI. | |
| 9 | A variant builder: define the variant and choose its data sources. | |
| 10 | A variant builder's explorer, and its tabs for its data sources and its build's results. | |
| 11 | Changing a data source's contents from the UI, seen by every variant builder using it. | |
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
| The Variant Builder's core | EDA placements, parts, part mappings, substitutions, assembly rules, load-outs and the BOM. | Next |
