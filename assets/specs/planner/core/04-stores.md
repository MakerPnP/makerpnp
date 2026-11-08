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
project's documents (`STORE-10`), the records of its data sources (`STORE-11`), the artifacts it generates
(`STORE-12`), and each phase's operation history (`STORE-13`), and the Variant Builder's core to read and write its
files (`STORE-14`), so neither reads or writes a file itself, which `clippy.toml` in each core's crate checks. *Target*:
the planner's core MUST be told when a file it read changes. The gap: only the stores core watches its data sources
(`STORE-9`).

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

**STORE-11 — A project's data sources.** *Confirmed.* The records of a project's data sources, its phases' load-outs,
its packages, its package mappings and its design variants' placements, MUST be read and written by the stores service,
as records the core uses, not as files, so that the core is tested with records kept in memory by its tests. An event
MUST be given the records it uses as they are when it's processed, read again for each event, so that a change made by
anyone is used. An event MUST change nothing, and write nothing, unless it succeeds: if it needs records that haven't
been read, what it changed MUST be undone, the records read, and the event processed again, with them, and if it fails,
what it changed MUST be undone, and the records it would have written MUST NOT be written. So that the event's work is
logged once, an event MUST ask for every record it can name before it changes anything, such as the placements of a
variant it's about to assign to a unit, and the records of a data source that doesn't exist are none, not an error, so
a phase can be created with a load-out that isn't there yet. An event that needs the records of a data source that
doesn't exist MUST say "`Data source not found. source: '<source>'`". The records an event changes MUST be written once
it succeeds, and a refused write MUST say why. A phase's load-out source MUST be resolved without checking that the
file exists, as the core does no IO. *Observed*: the operations are `ReadRecords` and `WriteRecords`, logged as the
service's others are (`STORE-7`), saying how many records there are, not what they are; the back end is `csv_file`,
which reads the CSV files as the core did, and writes load-outs, the only records the core changes. The service logs
"`Loading load-out. source: '<source>'`", "`Loading packages.`", "`Loading package mappings.`" or
"`Loading placements.`", and "`Storing load-out. source: '<source>'`", at info level. The core logs
"`planner_app: Reading records for the event. records: <records>`" at debug level when it asks for records, and is
given them as "`planner_app: Received event. event: WithRecords {`", the event to process again, then
"`planner_app: Stored records. source: '<source>'`" at info level once records are written. Creating a phase whose
load-out doesn't exist logs "`Created load-out. source: '<source>'`" and writes an empty one. The events that use them
are creating a phase, assigning placements to a phase, adding parts to a load-out, assigning a feeder, assigning a
variant to a unit, refreshing from the design variants, setting the placement ordering, generating the artifacts, and
viewing a phase's placements, its load-out, and the project's report. A relative load-out path MUST be relative to the
project's folder, in every event, as the phase keeps it as it was given (`ISSUE-7`). *Target*: an event that changes
the project and writes a load-out, such as assigning placements to a phase, keeps the project's change if the
load-out's write is refused, as the write's result comes after the event.

**STORE-12 — Artifacts.** *Confirmed.* The artifacts the planner's core generates, each phase's placements, the
project's JSON report, and, with the `markdown` feature, its markdown report, MUST be made by the core, as their
contents, and written by the stores service, as documents are (`STORE-10`), so that generating them is tested with no
files. They MUST be written in order, each phase's placements in the phases' order, then the report, and the first that
can't be written MUST say why, and stop the rest being written. *Observed*: each is written with `WriteDocument`. Once
each is written the core logs, at info level, "`planner_app: Generated phase placements. phase: '<phase>', path:
<path>`", "`planner_app: Generated JSON report. path: <path>`" or "`planner_app: Generated Markdown report. path:
<path>`", then, given "`planner_app: Received event. event: ArtifactsWritten {`", "`planner_app: Generated
artifacts.`". The markdown report is made from the JSON report's contents, not by reading its file. *Target*: the
project's change made by generating the artifacts, its phases' operation states, is kept if an artifact can't be
written, as the writes' results come after the event.

**STORE-13 — Operation histories.** *Confirmed.* Each phase's operation history, `<phase>_log.json` in the project's
folder, MUST be read and written by the stores service, as a document (`STORE-10`): recording a phase's operation, or
a placements' operation, MUST have the service read the history, then write it with the new items added, so that the
core makes and reads its contents, and is tested with no files. A history that doesn't exist MUST be created. The
histories MUST be written in order, and the first that can't be read or written MUST say why, and stop the rest.
*Observed*: the history's format is as it was, the items as JSON, indented by 4 spaces. Once each history is written
the core logs "`planner_app: Created operation history file. path: <path>`" or
"`planner_app: Updated operation history file. path: <path>`" at info level, then is given
"`planner_app: Received event. event: OperationHistoryWritten {`". *Target*: the project's change made by recording
the operation is kept if its history can't be written, as the write's result comes after the event.

**STORE-14 — The Variant Builder's files.** *Confirmed.* Building a variant MUST have the stores service read each of
its data sources, as records (`STORE-11`): the EDA placements, in the EDA tool's format, then each substitutions file,
in the order given, then the parts, the part mappings, the load-out, if there is one, and the assembly rules, if there
are any. The part mappings, substitutions and assembly rules MUST be read as each record's fields, by name, and made
by the core, as their rules can't be sent between the core and the service. The variant's placements, then its BOM, if
asked for, MUST be written by the stores service, as documents whose contents the core makes (`STORE-10`), so that the
core is tested with no files. A data source that doesn't exist MUST say
"`Data source not found. source: '<source>'`". The first data source that can't be read, or file that can't be
written, MUST say why, and stop the rest, so nothing is written if a data source can't be read. *Observed*: the files
are as they were, and the core logs as it did, "`Loaded <n> placements`" and so on, in the same order, once every data
source is read, then "`Output written to '<path>'`" and "`BOM written to '<path>'`" once each is written. The service
logs "`Loading <records>. source: '<source>'`" as it reads each, before them, as the core's reading of each no longer
comes between them.

**STORE-15 — Changing parts.** *Confirmed.* The core MUST change a data source's parts when asked (`ChangeParts`): add a
part, after the others, with its other values, set a part's other values, keeping those not named, or remove a part. A
change MUST say the revision of the parts it was based on, the revision of the view the user changed (`STORE-8`), and
MUST be refused if the data source has changed since, so that a change made by anyone else isn't lost: by the core, if
it has read a later revision, and by the stores service, if the data source's revision isn't the one the change was
based on, as when another process changed it, in which case the core MUST read it again, so every view shows the parts
as they are now, which the change can be made to again. The refusal MUST say "`The parts have changed since the change
was made.`", with the data source, the revision the change was based on and the data source's revision now, or none if
it no longer exists. Once the parts are written, the core MUST keep them, with the revision written, and send the data
source's parts view, so every view of it shows the change. A change MUST be refused, and nothing written, if the parts
aren't loaded, if another change to the data source is being written, as it would be based on the parts before it, if
the part added is there already, saying "`The part already exists.`", or if the part changed or removed isn't, saying
"`The part doesn't exist.`". A change that can't be written MUST say why, and keep the parts as they were. The data
source's status MUST say whether a change is being written, and why its last change was refused, until one succeeds,
even once the parts are read again, so every front end and view using it can show it. *Observed*: the operation is
`WriteParts`, logged as the service's others are (`STORE-7`), saying how many parts there are, not what they are. As a
data source keeps every value of every part, as a file's columns do, the core gives each part an empty value of each
name it has no value of, so the parts it keeps are as they'd be read back. The `csv_file` back end reads the file, and
compares its revision with the change's, then writes the parts, every value quoted, with the file's columns in the
order they were, and any new ones after them, in the order of their names, and logs
"`Storing parts. source: '<source>'`" at info level. A file that no longer exists isn't written again. The core logs
"`stores_app: Changed parts. source: '<source>', part_count: <count>, revision: '<revision>'`" at info level once the
parts are written. Once written, the service's watch says the data source changed (`STORE-9`), and the core reads it
again, finding its revision unchanged (`STORE-8`). *Target*: the service's check and its write MUST be one operation, so
a change made by another process between them isn't lost. The gap: they're two, though the time between them is short.

**STORE-16 — Changing parts from the CLI.** *Observed.* `planner_cli parts --source <file> add`, `set` and `remove`
MUST open the data source (`STORE-5`), then change its parts (`STORE-15`): `add` adds a part, `set` sets a part's
other values, keeping those not named, and `remove` removes a part, each naming the part with `--manufacturer` and
`--mpn`, and its other values with `--value <NAME>=<VALUE>`, once for each value. A change MUST be based on the
revision given with `--revision`, as logged when the parts were loaded, or else on the revision just read, so that a
change made by anyone since is refused. If the core refuses, the CLI MUST fail with the core's error, and the file is
left as it was.

**STORE-17 — Tables.** *Confirmed.* The core MUST show a data source's records as a table, read-only, when asked
(`OpenTable`), so a front end can show any data source before it can change it: its kind, its columns, in order, and
its rows, a view item for each record, in the order the data source has them. A view item has the domain's types, as a
parts view's has a part (`STORE-2`), so each value is shown as what it is: a design's EDA placements are
`EdaPlacement`s, in the EDA tool's format, part mappings an EDA tool, a `Pattern` for each field of any EDA tool the
data source has a column for, not only its own tool's, in the EDA tools' order, so every value is shown, and can be
changed, whichever tool the row has, as a row's tool may be changed (`VBGUI-13`), and a `Part`, substitutions, likewise,
an EDA tool, a `Pattern` for each field of any EDA tool the data source has a pattern's column for, and a value for
each it has a value's column for, not only its own tool's, in the EDA tools' order (`VBGUI-14`), and a comment, assembly rules `AssemblyRule`s, and a variant's placements and BOM, as built (`STORE-14`), `Placement`s,
and each part's designators, as `RefDes`es, footprint, JLCPCB part, quantity and comment. A parts data source has its
parts view (`STORE-2`). A pattern is a regular expression if it starts and ends with "`/`", the expression being
between them, or else a value matched exactly, as the Planner and the Variant Builder match them, and is parsed once,
as the data source is read, so it's ready to be shown, and used, as often as needed; one that isn't valid is kept, with
why, so it's shown as it is. A record that can't be read, such as one whose EDA tool isn't known, MUST be refused,
saying why. The columns are worked out once, as the table is read, named as the data source names them, the EDA
tool's fields, and those matched, in the order they're first found, and a comment's column last. A table MUST be
opened, identified, loaded
once, and watched, as parts are (`STORE-5`, `STORE-6`, `STORE-8`, `STORE-9`), and a refused read MUST say why and keep
the rows loaded before (`STORE-3`), by its kind and its data source, each opened and read on its own, so a data source
shown as a table and as parts is read for each. A data source that doesn't exist, such as a BOM not yet built, MUST say
"`Data source not found. source: '<source>'`", and is still watched, so it's shown once it's written. *Observed*: the
operation is `ReadTable`, logged as the service's others are (`STORE-7`), saying how many rows there are, not what
they are. The `csv_file` back end reads each record as the core that uses it does: part mappings' fields' patterns by
their names, in any case, as the part mapper does, and substitutions' as "`<Field>Pattern`" and "`<Field>`", with
"`Comment`", as the Variant Builder does, by their names as they're written. A substitution whose tool's field has no
pattern's column, or no value's column, is read, without it, so the row can be shown, and given the column
(`STORE-21`), though the Variant Builder's build refuses it. Part mappings' columns are made from the file's headers, a
pattern's column for each EDA tool's field it has a column for, in the EDA tools' order, so they're there even with no
rows, and substitutions' likewise, a pattern's column for each field it has a pattern's column for, and a value's for
each it has a value's column for, each field's pattern's before its value's, and a comment's last, if it has one. A BOM is
written without headers, as JLCPCB's are, so every row is read, by
the order of its values, with the columns "`Designator`", "`Footprint`", "`JLCPCB Part`", "`Quantity`" and
"`Comment`", its designators joined by "`, `", and a quantity as a number. Part mappings' columns are "`Eda`", each
field's, such as "`Name`" and "`Value`", "`Manufacturer`" and "`Mpn`", and substitutions' "`Eda`", each field's
pattern's and value's, such as "`NamePattern`" and "`Name`", and "`Comment`". It reads EDA placements as the Variant
Builder reads them, with the columns "`RefDes`", "`Place`", "`Side`", "`X`", "`Y`", "`Rotation`", then each field's
name, as the Variant Builder names them, such as "`name`" and "`value`", in the order they're first found, and each
side as the Variant Builder names it, such as "`top`", logging "`Loading <kind>. source: '<source>'`"
as `STORE-11`'s reads do. The view is sent as a `TableView` effect, and the core's view model says each table's status,
by its kind and its data source as it was asked for. The core logs
"`stores_app: Loaded table. kind: <kind>, source: '<source>', row_count: <count>, revision: '<revision>'`",
"`stores_app: Table already loaded. kind: <kind>, source: '<source>'`" and
"`stores_app: Table unchanged. kind: <kind>, source: '<source>', revision: '<revision>'`" at info level, and
"`stores_app: Data source changed. source: '<source>', kind: <kind>`" for each change. Matching, by the Planner and
the Variant Builder, uses only the row's tool's fields. The EDA tools' fields are the domain's, so the cores, the
stores service and the readers share them: DipTrace's "`name`" and "`value`", KiCad's "`package`" and "`val`", and
EasyEDA's "`device`" and "`value`", in that order, a field shared by tools, such as "`value`", being one field.

**STORE-18 — Listing a table from the CLI.** *Observed.* `planner_cli table --kind <kind> --source <file> list`, with
`--eda-tool <tool>` for EDA placements, MUST open the table (`STORE-17`) and write it to standard output as CSV, every
value quoted, with its columns' names as the headers, and each value as its column shows it, a pattern as the data
source has it. The kinds are `eda-placements`, `part-mappings`, `substitutions`,
`assembly-rules`, `placements` and `bom`. If the core refuses, the CLI MUST fail with the core's error.

**STORE-19 — Changing a table.** *Confirmed.* The core MUST change a table's rows when asked (`ChangeTable`), so the
user changes a data source where it's shown: update a row's values, add a row, after the others, remove a row, or move a
row to before another, or to the end. As a data source may later be a database, or a web API, whose queries may return
some of a table's columns, or columns from many tables (the Variant Builder mission's "Architecture"), a change MUST name
rows and columns as the data source does, not by their places in the view: a table's view MUST have each row's key,
made by the back end, and each column's reference, as the data source names it, and say how its rows are ordered, not
at all, by their place in the data source, or by a column's values. A front end changes a row as it's shown, giving
the row as it was and as it's to be, and the core MUST make the change the back end makes: the row's key, and, for each
of its columns whose value changed, the column's reference, its value as it was, and its value as it's to be, each as
the data source has it, such as a pattern's text. The back end MUST change only those columns, and only if each value
is still as it was, so a column the table doesn't show is never touched, and a change to one row doesn't refuse a change
to another. Removing or moving a row MUST check each of its values, and moving one before another the other's too, so
the rows are still those the user saw. A change whose values aren't as they were, or whose row isn't there, MUST be
refused, saying "`The table has changed since the change was made.`", with its kind, its data source and its revision
now, or none, and the core MUST then read the table again, so every view shows it as it is now, which the change can be
made to again. A change MUST NOT be checked against the table's revision, which says whether a view is out of date.
Once the back end has changed the data source, the core MUST read the table again, and send its view, so every view of
it shows the change, with the keys the back end gives it now. A change MUST be refused, and nothing changed, if the
table isn't loaded, if another change to it is being made, if its kind can't be changed, saying
"`The table can't be changed.`", if a value is in a column the data source doesn't have, saying
"`The column doesn't exist.`", or if a row is moved and the table's rows aren't ordered, saying
"`The rows can't be moved.`". A change that can't be made MUST say why, and keep the table as it was. The table's
status MUST say whether a change is being made, and why its last change was refused, until one succeeds. Part mappings
and substitutions can be changed, a substitution's values being its EDA tool, its patterns, the values its fields are
substituted with, and its comment, empty if it has none; the other kinds are to come (the Variant Builder mission's
step 11). A pattern that isn't a valid
regular expression is written as it is, as it's the user's, and it's shown as it is (`STORE-17`). *Observed*: the
operation is `ChangeRows`, logged as the service's others are (`STORE-7`). The `csv_file` back end's key of a row is its
place in the file, counted from 0, when it was read, so it's checked with the row's values, its references are the
file's headers, as they're written, and its rows are ordered by their place. It reads the file, checks the change's
values, makes the change, and writes the file, every value quoted, its columns as they were, logging
"`Changing part mappings. source: '<source>'`", or "`Changing eda substitutions. source: '<source>'`", at info level.
A row's EDA tool is written as `STORE-17` names it, such as "`DipTrace`". The core logs
"`stores_app: Changed table. kind: <kind>, source: '<source>', revision: '<revision>'`" at info level once the data
source is changed. *Target*: a front end showing some of a table's rows, such as a page of them, and the back end
working out the ordering values of a table ordered by a column's values.

**STORE-20 — Changing a table from the CLI.** *Confirmed.* `planner_cli table --kind part-mappings --source <file>`,
and `--kind substitutions`, `add`, `set`, `remove` and `move` MUST open the table (`STORE-17`), then change it (`STORE-19`), so the CLI can change
a data source as the GUI does. `list --keys` MUST list each row's key, as the data source gives it, in a first column,
"`Key`", by which the other commands name a row with `--row <KEY>`. `add` adds a row, after the others, with its EDA
tool, `--eda <TOOL>`, its manufacturer, `--manufacturer`, and MPN, `--mpn`, and a pattern for each field named with
`--pattern <FIELD>=<PATTERN>`, once for each field, such as "`name=/R.*/`"; a substitution, instead of a part, has a
value for each field named with `--value <FIELD>=<VALUE>`, such as "`name=RES_0402`", and a comment, `--comment`, and
`--manufacturer` and `--mpn` are refused for it, as `--value` and `--comment` are for a part mapping. `set` sets the row's values named with the
same options, keeping those not named, and needs at least one. `remove` removes the row, and `move` moves it to before
the row named with `--before <KEY>`, or to the end with `--to-end`. The row is changed as it was just read, so a change
made by anyone between the read and the change is refused (`STORE-19`); to refuse one made since the user saw the row,
such as with `list`, its values as they saw them MAY be given with `--was-eda`, `--was-pattern <FIELD>=<PATTERN>`,
`--was-manufacturer` and `--was-mpn`, or, for a substitution, `--was-value <FIELD>=<VALUE>` and `--was-comment`,
and the change is checked against those. As `STORE-19` checks only the values
an update changes, `set`'s `--was-` values are checked only for the values it sets. A row that isn't there MUST fail,
saying "`The row doesn't exist. key: '<key>'`". If the core refuses, the CLI MUST fail with the core's error, and the
file is left as it was. The other kinds are to come with the core's (the Variant Builder mission's step 11), and are
refused, as the core refuses them.
`add-columns` MUST add columns to the table (`STORE-21`): `--eda <TOOL>` the columns the EDA tool needs that the data
source doesn't have, as the core works them out, or `--column <NAME>`, once for each column, with where they go,
`--first`, `--last`, `--before <COLUMN>` or `--after <COLUMN>`, a column named as the data source names it, which
`list` shows for the columns it lists; named columns need where they go, and an EDA tool's take none. Each says which
columns were added, "`Added columns. columns: ['<column>', ...]`", an EDA tool's being those the table shows once
they're added that it didn't, or that none were needed, "`No columns needed.`", on standard output. A row's EDA tool
is changed, or a row of another tool added, with `set` or `add` after its tool's columns are added; a pattern, or a
value, for a field the data source has no column for fails, as `STORE-19` says.

**STORE-21 — Adding a table's columns.** *Confirmed.* The core MUST add columns to a table when asked
(`AddTableColumns`), so a user can give a row values the data source has no columns for, such as a part mapping's
patterns for another EDA tool's fields. It's its own change, not part of a change to rows (`STORE-19`), so each back
end has one change of each kind to make, as there are no transactions: a front end that needs both adds the columns,
then, once the table's read again, changes the rows. A column added that's then not used is kept, as a column no row
uses is (`VBGUI-13`). A front end MUST be able to ask for columns in either of two ways:

- **Named columns**: each column's name, as the data source is to name it, such as a CSV file's header, and where the
  columns go: first, last, before a column, or after a column, the column named as the data source names it, as the
  table's columns' references do (`STORE-19`), so the core never needs to know a data source's names. The columns are
  added in the order given, together. A column has no kind yet; each back end adds it as its plainest, a CSV file's
  text.
- **An EDA tool's columns**: the core MUST work out which of the tool's fields' columns the data source doesn't have,
  so a column shared by tools, such as DipTrace's and EasyEDA's "`value`", is added once, and used by both, and add
  those: for part mappings, each named as the field, in upper camel case, such as "`Package`", and for substitutions,
  each field's pattern's column, "`<Field>Pattern`", such as "`PackagePattern`", and its value's, "`<Field>`", such as
  "`Package`", each that it doesn't have, the patterns' first, then the values', each in the tool's order. The core
  works them out from the table's columns, whose patterns' and values' columns are those of every EDA tool's field the
  data source has, even with no rows (`STORE-17`). Where they go in the data source is the back end's to say, as only
  it knows the order of the data source's columns, which needn't be the table's (`STORE-17`). For part mappings:
  1. If the data source has a "`Manufacturer`" column, or an "`Mpn`" column, or both, and every pattern's column is
     before them, they go after the last pattern's column, or, if it has none, before the first of them.
  2. Otherwise, they go after the last column.

  For substitutions, whose patterns' columns, and values' columns, are each a group, which may be among other columns:
  1. The patterns' columns go after the last pattern's column, and the values' after the last value's column.
  2. If it has no pattern's column, they go before the first value's column, and if it has no value's column, after
     the last pattern's column.
  3. If it has neither, the patterns', then the values', go before its "`Comment`" column, or, if it has none, after
     the last column.

  If none are needed, nothing is changed, and the core logs
  "`stores_app: Table columns already there. kind: <kind>, source: '<source>', eda_tool: <tool>`" at info level.

The back end MUST add the columns where they're asked to go, with an empty value for each row, leaving every other
column, value and row as it was, so the rows' keys are as they were. Adding columns MUST be refused, and nothing
changed, if the table isn't loaded, if a change to it is being made (`STORE-19`), if its kind can't be changed (`STORE-19`), if a
column to add is already there, saying "`The column already exists. column: '<column>'`", if a column is to go before
or after one the data source doesn't have, saying "`The column doesn't exist.`", or if the back end can't add columns,
saying why. Whether a column is there is the back end's to say, as only it knows every column the data source has,
including those the table doesn't show. A data source that's gone MUST say "`Data source not found.`", and the core
MUST read the table again, so every view shows it's gone. A refusal is kept, and shown, as a change's is, until a
change succeeds (`STORE-19`). Once the columns are added, the core MUST read the table again, and send its view, so
every view of it shows them. *Observed*: the operation is `AddColumns`, logged as the service's others are
(`STORE-7`), with the columns and where they go, `First`, `Last`, `Before("<column>")`, `After("<column>")`, or
`EdaToolColumns` for an EDA tool's. The
`csv_file` back end reads the file, checks the columns, a column being there if a header has its name in any case, as
the part mapper finds a field's column, and one to go by being named by its header as it's written, as a column's
reference is, a part mapping's pattern's column being one whose header is an EDA tool's field's name in any case, and
the part's "`Manufacturer`" and "`Mpn`" as they're written, and a substitution's pattern's column "`<Field>Pattern`",
and its value's "`<Field>`", for an EDA tool's field, as they're written, as the Variant Builder reads them, inserts each header where it's to go, with an empty value in each row, and writes the file, every value
quoted, logging "`Adding columns. source: '<source>', columns: ['<column>', ...]`" at info level. The core logs
"`stores_app: Added table columns. kind: <kind>, source: '<source>', columns: ['<column>', ...], revision: '<revision>'`"
at info level once they're added. Part mappings' and substitutions' columns can be added; the other kinds are to come with their changes
(the Variant Builder mission's step 11).

## Acceptance criteria

- `planning/stores_app/tests/tables.rs`, with no files, checks opening a table identifies its data source, reads it,
  watches it, and sends its view, with its kind, columns and rows in order, a BOM's comment last, that opening it again
  reads nothing, that a
  change reads it again, and an unchanged one sends no view, that a refused read says why and keeps the rows, that a
  data source that doesn't exist says so, and is shown once a change finds it, that the view model says each table's
  status, and that the result of a different operation is refused (`STORE-17`).
- `planning/stores_service/tests/csv_file.rs` reads part mappings, with an exact pattern, a valid regular expression,
  parsed, and one that isn't, kept, reads part mappings with a pattern for each field of every EDA tool the file has a
  column for, in the EDA tools' order, whatever the columns' order and the row's tool, refuses part mappings whose EDA
  tool isn't known, reads substitutions, with their
  comment last, assembly rules, a variant's placements, a BOM without headers, with its designators, and its comment
  last, and EDA placements, checking each table's columns and values, in order, and checks a file that doesn't exist is
  none (`STORE-17`).
- `common/criteria`'s tests check a value between "`/`"s is a regular expression, one that
  isn't valid says why, any other value is matched exactly, a lone "`/`" is an empty expression, as it was, and a
  pattern is serialized as the data source has it, and parsed again (`STORE-17`).
- `planning/planner_cli/tests/tables.rs` lists a BOM, without headers, with JLCPCB's columns, the comment last, and EDA
  placements, checks
  the output, and checks
  "`stores_app: Received event. event: OpenTable {`",
  "`stores_service: Performing stores operation. operation: ReadTable {`" and
  "`stores_app: Loaded table. kind: Bom, source: '`" are logged, in order, with no "`Event failed.`", and that a file
  that doesn't exist fails, saying so (`STORE-18`).
- `planning/stores_app/tests/tables.rs` checks a table's view has its rows' keys, its columns' references and its
  ordering, that a change to part mappings has the service update a row's changed columns only, by its key and their
  references, with their values as they were and are to be, add a row's values, remove a row, and move a row before
  another, or to the end, checking their values, then reads the table again and sends its view, that the status says
  a change is being made, that a table that isn't loaded, a table of a kind that can't be changed, a column the data
  source doesn't have, and moving a row of a table that isn't ordered, are refused, changing nothing, that one the
  service refuses, as the table changed, reads it again, and keeps the change's error until a change succeeds, that one
  that can't be made says why and keeps the table, and that a change while another is made is refused (`STORE-19`).
- `planning/stores_service/tests/csv_file.rs` reads part mappings' keys, references and ordering, then updates a row,
  adds a row, removes a row and moves a row, before another and to the end, checking the file, its columns as they
  were, every value quoted, a column the table doesn't show untouched, and the revision the file's, and checks a
  change whose values aren't as they were, or whose row isn't there, changes nothing, the refusal saying the file's
  revision now (`STORE-19`).
- `planning/planner_cli/tests/tables.rs` lists part mappings with their keys, then adds a row, sets a row's values,
  moves a row before another and to the end, and removes a row, checking the file after each, every value quoted, and a
  column the table doesn't show untouched, and checks "`stores_app: Received event. event: ChangeTable {`",
  "`stores_service: Performing stores operation. operation: ChangeRows {`" and
  "`stores_app: Changed table. kind: PartMappings, source: '`" are logged, in order, with no "`Event failed.`". It
  checks a change whose `--was-` values aren't the row's, a row that isn't there, and a table that can't be changed,
  fail, saying why, and leave the file as it was (`STORE-20`).
- `planning/planner_cli/tests/tables.rs` adds EasyEDA's columns to DipTrace's part mappings, only "`Device`", as
  "`Value`" is there, after "`Value`", the last pattern's column, saying so, then adds an EasyEDA row, checks DipTrace's need none, and adds
  named columns after another, checking the file and what's said after each, and checks
  "`stores_app: Received event. event: AddTableColumns {`",
  "`stores_service: Performing stores operation. operation: AddColumns {`", "`Adding columns. source: '`",
  "`stores_app: Added table columns. kind: PartMappings, source: '`" and
  "`stores_app: Table columns already there. kind: PartMappings, source: '`" are logged, in order, with no
  "`Event failed.`". It checks a pattern of a field without a column, a column there already, in any case, one to go by
  that isn't there, named columns without where they go, an EDA tool's with where they go, and adding columns to a BOM,
  fail, saying why, and leave the file as it was (`STORE-20`, `STORE-21`).
- `planning/planner_cli/tests/tables.rs`'s `an_eda_tools_columns_are_added_last_if_the_parts_are_not_after_the_patterns`
  adds KiCad's, then EasyEDA's, columns to a file whose "`Manufacturer`" and "`Mpn`" are before "`Name`" and
  "`Value`", checking they're added after its last column, EasyEDA's after KiCad's, every other column where it was,
  and checks "`position: EdaToolColumns`" is logged for each, with no "`Event failed.`" (`STORE-21`).
- `planning/stores_app/tests/tables.rs` checks named columns are added where they're asked to go, then the table is
  read again and its view sent, that an EDA tool's columns are those of its fields the table doesn't have, whichever
  tool's they are, so a column shared by tools, such as "`Value`", is added once, where the data source puts an EDA tool's
  columns, `EdaToolColumns`, that
  none are added, and nothing asked of the service, if the table has them all, that adding columns to a table that
  isn't loaded, of a kind that can't be changed, or while a change is being made, is refused, that a column the service
  says is there already, a column to go by that isn't, and columns it can't add, say why, keep the table, and keep the
  error until a change succeeds, and that a data source that's gone says so and is read again (`STORE-21`).
- `planning/stores_service/tests/csv_file.rs` adds columns first, last, before and after a column, together, in the
  order given, to a file with rows and one with none, checking the file, every other value as it was, and the
  revision, adds an EDA tool's columns after the last pattern's column of files whose "`Manufacturer`" or "`Mpn`", or
  both, are after every pattern's column, a column of the user's between them, before them if there's no pattern's
  column, and after the last column of files whose part's columns are before a pattern's, or that have none, refuses a
  column there already, in any case, one given twice, and a column to go by that isn't there, the file left as it was, says a file that isn't there is gone, and reads part mappings with no rows with their fields'
  columns (`STORE-21`, `STORE-17`).
- `planning/stores_service/tests/csv_file.rs` reads substitutions with a pattern and a value for each field of every
  EDA tool the file has a column for, in the EDA tools' order, whatever the columns' order and the row's tool, a row
  whose tool's field has no pattern's or value's column without it, and substitutions with no rows with their fields'
  columns, and a comment's last, its keys, references and ordering, then updates a row, its tool, a pattern, a value
  and its comment, adds, removes and moves a row, checking the file after each, and adds an EDA tool's columns to files
  whose patterns' and values' columns are in two groups, among other columns, in pairs, each field's pattern's then its
  value's, with only one group, and with neither, with and without "`Comment`", checking each column goes where
  `STORE-21` says (`STORE-17`, `STORE-19`, `STORE-21`).
- `planning/stores_app/tests/tables.rs` checks a change to substitutions has the service update a row's changed
  columns, its tool, patterns, values and comment, by their references, add, remove and move a row, and that an EDA
  tool's columns for substitutions are its fields' patterns' and values' columns the table doesn't have, the patterns'
  first, where the data source puts them, `EdaToolColumns` (`STORE-19`, `STORE-21`).
- `planning/planner_cli/tests/tables.rs` lists substitutions with their keys, then adds a row with `--pattern`,
  `--value` and `--comment`, sets its values, checked with `--was-value` and `--was-comment`, moves and removes a row,
  adds KiCad's columns, then a KiCad row, checking the file after each, every value quoted, and checks
  "`stores_app: Changed table. kind: Substitutions, source: '`" and
  "`stores_app: Added table columns. kind: Substitutions, source: '`" are logged, in order, with no "`Event failed.`".
  It checks `--manufacturer` for a substitution, and `--value` for a part mapping, fail, saying why, and leave the
  file as it was (`STORE-20`, `STORE-21`).

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
- `planning/stores_app/tests/parts.rs` checks a change has the service write the parts, changed, based on the same
  revision, and sends the view with the revision written, that a part added with other values than the others' has
  empty ones, as every part has, that setting a part's values keeps those not named, and removing a part keeps the
  others in order, that adding a part twice, or changing or removing one that isn't there, is refused, writing nothing,
  that a change based on an older revision than the core's is refused, writing nothing, that one the service refuses,
  as the parts changed, reads them again, sends their view, and keeps the change's error until a change succeeds, that
  one that can't be written says why and keeps the parts, and that a change while another is written is refused
  (`STORE-15`).
- `planning/stores_service/tests/csv_file.rs` checks parts are written with the file's columns in the order they were,
  new ones after them, every value quoted, with the revision of the file as it's read back, and that a file changed, or
  removed, since the change's revision isn't written, the refusal saying its revision now, or none (`STORE-15`).
- `planning/planner_cli/tests/parts.rs` adds a part, sets a part's values and removes a part, checking the file, with
  its columns in the order they were, and that
  "`stores_app: Received event. event: ChangeParts {`",
  "`stores_service: Performing stores operation. operation: WriteParts {`",
  "`stores_service::csv_file: Storing parts. source: '`", "`stores_app: Received event. event: PartsWritten {`" and
  "`stores_app: Changed parts. source: '`" are logged, in order, with no "`Event failed.`". It checks a change based on
  another revision, given with `--revision`, fails, saying the parts have changed, with nothing written, and that a part
  that's there already isn't added again (`STORE-15`, `STORE-16`).
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
- `planning/planner_app/tests/documents.rs`, with no files, answers the planner's core's reads and writes of records
  itself, and checks creating a phase reads its load-out and writes an empty one if there's none, that one whose
  load-out exists keeps it, and that each event reads the records again. It checks a phase whose load-out can't be read
  isn't created, and changes nothing, that assigning a feeder writes the load-out with the feeder assigned, that a
  feeder that can't be assigned writes nothing, that a load-out that can't be written says why, that a load-out that
  doesn't exist says so when it's viewed, and that assigning a variant to a unit reads the variant's placements first,
  once (`STORE-11`).
- `planning/planner_app/tests/documents.rs` checks generating the artifacts writes the phase's placements, then the
  JSON report, ending with a new line, then, with the `markdown` feature, the markdown report, and that an artifact
  that can't be written says why, and the report after it isn't written (`STORE-12`).
- `tests/ui/workflow.rs`, in the GUI, generates the artifacts, and checks
  "`planner_app: Received event. event: GenerateArtifacts`",
  "`stores_service: Performing stores operation. operation: WriteDocument { path: `",
  "`planner_app: Generated phase placements. phase: 'top_1', path: `", the same for `bottom_1`,
  "`planner_app: Generated JSON report. path: `", "`planner_app: Generated Markdown report. path: `",
  "`planner_app: Received event. event: ArtifactsWritten { result: Ok(()) }`" and
  "`planner_app: Generated artifacts.`" are logged, in order, with no "`Event failed.`", and that the files are as the
  CLI's. The CLI's `sequence_16_generate_artifacts` checks the "`Generated`" lines in order, and the files, as before
  (`STORE-12`).
- `planning/planner_app/tests/documents.rs`, with no files, checks recording a task's operation reads the phase's
  operation history, then writes it with the task added, creating it if there's none, and that a history that can't be
  read says why, and writes nothing (`STORE-13`).
- `tests/ui/operations.rs`, in the GUI, abandons a task, and checks
  "`planner_app: Received event. event: RecordPhaseOperation {`",
  "`stores_service: Performing stores operation. operation: ReadDocument { path: `",
  "`stores_service: Performing stores operation. operation: WriteDocument { path: `",
  "`planner_app: Updated operation history file. path: `" and
  "`planner_app: Received event. event: OperationHistoryWritten { result: Ok(()) }`" are logged, in order, with no
  "`Event failed.`" (`STORE-13`).
- `planning/variantbuilder_app/tests/variant_builders.rs`, with no files, answers the Variant Builder's core's reads and
  writes itself, and checks building reads each data source in order, then writes the placements and the BOM, as CSV,
  that a data source that doesn't exist says so, the data sources after it aren't read, and nothing is written, and
  that placements that can't be written say why, and the BOM isn't written (`STORE-14`).
- `planning/variantbuilder_cli/tests/variantbuilder.rs` builds variants of DipTrace, KiCad and EasyEDA designs, and
  checks the "`Loaded`" lines are logged in order, and that the placements are as before (`STORE-14`, `VARBCLI`).
- `planning/planner_cli/tests/planner.rs` checks "`Created load-out. source: '`" is logged before the phase is created,
  that "`Loading load-out. source: '`" is logged before the placements are assigned to the phase, and
  "`Storing load-out. source: '`" after, and that the load-out is as before (`STORE-11`). It creates a phase with a
  relative load-out from a folder above the project's, and checks the load-out is created in the project's folder,
  with "`Created load-out. source: '`" naming it (`ISSUE-7`, `STORE-11`).
- `tests/ui/workflow.rs`, in the GUI, assigns a feeder, and checks
  "`planner_app: Received event. event: AssignFeederToLoadOutItem {`",
  "`planner_app: Reading records for the event. records: `",
  "`stores_service: Performing stores operation. operation: ReadRecords { kind: LoadOut, source: `",
  "`planner_app: Received event. event: WithRecords {`", "`Assigned feeder to load-out item. feeder: FEEDER_1`",
  "`stores_service: Performing stores operation. operation: WriteRecords { source: `" and
  "`planner_app: Stored records. source: `" are logged, in order, with no "`Event failed.`", and that the load-out is
  as the CLI's (`STORE-11`).
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
