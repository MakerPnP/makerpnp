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

## Steps

| Step | What | Status |
|---|---|---|
| 1 | The stores core (`planning/stores_app`): load a parts data source, a CSV file for now, and view its parts. | Done |
| 2 | The CLI uses the stores core: `planner_cli parts --source <file> list`. | Done |
| 3 | "Open parts" in the GUI: choose a parts data source, and show its parts in a table, through the stores core. A data source shown by many tabs is loaded once, and kept once, and reloading it in one tab updates them all. | Done |
| 4 | A variant builder: define the variant and choose its data sources. | Next |
| 5 | A variant builder's explorer, and its tabs for its data sources and its build's results. | |
| 6 | Changing a data source's contents from the UI, seen by every variant builder using it. | |
| 7 | Placements, shopping list and BOM tables, linked to each other and to the PCB. | |
| 8 | Confirming BOM items. | |
