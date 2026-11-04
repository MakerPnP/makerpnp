# PCB assembly

What the MakerPnP Planner models: how components get attached to a PCB, and where each MakerPnP tool fits. Read this
before reasoning about processes, operations, tasks or phases, or before writing an example, preset, test or doc that
describes an assembly job.

## The MakerPnP tools

MakerPnP is a family of tools, each with its own name. Name the tool you mean, for example "the MakerPnP Planner", not
"MakerPnP".

- In this repository: the **Planner** (core, CLI and GUI), the **Variant Builder** (EDA placements and BOMs to
  planner input), and the **Gerber Viewer**.
- In the `machine` repository (`../machine`, and the MakerPnP organization on GitHub): the machine's **firmware** and
  the **operator UI** for running the machine.

The MakerPnP Planner plans and tracks PCB assembly jobs, with or without a pick-and-place machine. Its first users are
makers doing small-volume assembly, but it isn't limited to them. It is the job's "slicer": it turns EDA output
(placements, BOM) and the user's parts into phases, load-outs, placement orderings and artifacts, then records progress
as the job is built.

## Attaching components

A board is assembled by attaching each component with one **attachment method**. Each method is its own sequence of
steps, and only some of them involve solder paste or a reflow oven:

| Method | Typical steps |
|---|---|
| Manual placing and hand soldering | Load the PCB, then place and solder each part by hand. No solder paste and no reflow. |
| Solder paste and reflow | Apply paste (stencil, dispenser or by hand), place the parts by machine or by hand, then reflow (oven, hot plate or hot air). |
| Glue and wave soldering | Dispense glue, place the parts, cure the glue, turn the board over, then wave solder. Often bottom-side SMT mixed with through-hole. |
| Through-hole | Insert the parts by hand or machine, then hand, wave or selective solder. |
| Straddle-mount | Mount the PCB in a jig, place the components in the jig, slide the jig into place, solder the top, then solder the bottom. The component is soldered on both sides of the PCB. Not yet supported (`PROCDEF-9`). |
| Other machines | Another placement machine, a soldering robot, or any other equipment, each with its own steps and load-out. |

One board often needs several methods, in an order that matters, for example top-side paste and reflow, then top-side
through-hole by hand. Inspection and rework can follow any of them.

A **pick-and-place machine** is one way to place parts, used within whichever method the job needs. It picks parts from
feeders with a vacuum nozzle, aligns them with cameras, and puts them at the X/Y and rotation from the placement file.
Which part is in which feeder is the **load-out**; changing it is slow manual work, which is why the variant builder
prefers parts already loaded, and why phases have load-outs and orderings.

## What this means for the planner

- The user defines each process as ordered operations and tasks; the planner models every method above the same way.
  Only the task kinds in `PROCDEF-2` (`specs/planner/core/02-processes.md`) get special treatment.
- A phase uses one process for a set of placements, usually one side of the board. A job with several methods has a
  phase for each. Straddle-mount components, soldered on both sides, are a goal with its own process (`PROCDEF-9`).
- The `pnp` and `manual` presets are starting points, matching the README's examples. An example or test for another
  method uses that method's own steps from the table.

## Around the planner

Before planning: design in an EDA tool, fabrication and panelisation, then mapping the BOM to real parts (the variant
builder). After assembly: inspection, rework, depanelisation, cleaning, programming and testing. The planner covers
planning the job and tracking it through assembly.
