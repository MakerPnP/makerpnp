<div align="center">

[![Build Status](https://github.com/makerpnp/makerpnp/workflows/Rust/badge.svg)](https://github.com/makerpnp/makerpnp/actions/workflows/rust.yml)
[![Discord](https://img.shields.io/discord/1255867192503832688?label=MakerPnP%20discord&color=%2332c955)](https://discord.gg/ffwj5rKZuf)
[![YouTube Channel Subscribers](https://img.shields.io/youtube/channel/subscribers/UClzmlBRrChCJCXkY2h9GhBQ?style=flat&color=%2332c955)](https://www.youtube.com/channel/UClzmlBRrChCJCXkY2h9GhBQ?sub_confirmation=1)
[![MakerPnP GitHub Organization's stars](https://img.shields.io/github/stars/makerpnp?style=flat&color=%2332c955)](https://github.com/MakerPnP)
[![Donate via Ko-Fi](https://img.shields.io/badge/Ko--Fi-Donate-green?style=flat&color=%2332c955&logo=ko-fi)](https://ko-fi.com/dominicclifton)
[![Subscribe on Patreon](https://img.shields.io/badge/Patreon-Subscribe-green?style=flat&color=%2332c955&logo=patreon)](https://www.patreon.com/MakerPnP)


![MakerPnP](assets/logos/makerpnp_icon_1_384x384.png)

</div>

# MakerPnP

Cross-platform Pick-and-Place machine software, for Makers!

## Project Introduction

[![MakerPnP - Project Introduction](http://img.youtube.com/vi/s9yh92Ctqh8/0.jpg)](http://www.youtube.com/watch?v=s9yh92Ctqh8 "MakerPnP Project Introduction")

This project is work-in-progress, you can follow development using the links in the [links](#links) section below. 

## Project Status

Development is currently focused on the machine operations. Please see the dedicated repo here: https://github.com/MakerPnP/machine

In 2024/Q3 to 2025/Q2 development was focused on the workflow side of PCB assembly, UI selection and gerber files (types, parser, viewer).
Currently, a pure-rust desktop GUI for the planner is in a usable state, but un-polished.  Additionally, there are currently two functional cli tools,
planner_cli and variant builder and a stand-alone pure-rust gerber viewer.

## Planning

A PCB assembly job generally consists of the following things:

1) One or more PCBs that need populating, which may need populating on a single side or both sides or on rigid-flex layers.
2) A list of placements for each EDA design variant to be populated. 
3) A list of manufacturer + part codes for each part used in the job. (aka Bill-of-materials/BOM)
4) One or more processes.
4) A set of operations for each process.
5) A set of tasks for each operation.
6) One or more phases.
7) A set of parts for each phase.
8) One or more load-outs.
9) A defined placement ordering for each phase.

The processes are defined by the user, so that the planner works however components are attached: a pick-and-place
machine and reflow oven, placing and soldering by hand, applying glue and wave soldering, other machines or soldering
robots, or any mix of these. As with a PCB, each process is created in its own file, `<name>.process.json`, and then
added to a project, which keeps a copy of it until its processes are refreshed. A process can start from one of the
presets, `pnp` or `manual`, which match the examples below. Each task has a name chosen by the user, and a kind the
planner understands: generic, loading PCBs, or placing components. See
[`assets/specs/planner/core/02-processes.md`](assets/specs/planner/core/02-processes.md), and
[`assets/pcb-assembly.md`](assets/pcb-assembly.md) for the ways components are attached.

For example, a manual placement job for a double-sided 2x2 panel of PCBs with 2 designs, each design having 2 assembly
variants might be defined as follows:

```
Project
├── Processes
│   └── Process = Manual
│       └── Operations:
│           ├── 1
│           │   └── Tasks
│           │       └── Load PCBs
│           └── 2
│               └── Tasks
│                   ├── Place components
│                   └── Manually solder components
├── PCBs
|   └── 1 (Panel)
|       ├── Unit 1 (x=1, y=1) - Design A (Variant A) 
|       ├── Unit 2 (x=2, y=1) - Design A (Variant B)
|       ├── Unit 3 (x=1, y=2) - Design B (Variant A)
|       └── Unit 4 (x=2, y=2) - Design B (Variant B)
└── Phases
    ├── 1 - Top
    │   ├── Process = Manual    
    │   ├── Ordering: Panel Unit:Ascending, Part Cost:Ascending, Part Area:Ascending, Quantity:Desc
    │   └── Loadout: 'desk-top'.
    └── 2 - Bottom
        ├── Process = Manual
        ├── Ordering: Panel Unit:Ascending, Part Cost:Ascending, Part Area:Ascending, Quantity:Desc
        └── Loadout: 'desk-bottom'.
```

Similarly, the job for a single-sided 4x1 panel having a single design variant, using a mixed assembly
processes (SMT + TH) might be defined as follows:

```
Project
├── Processes
│   ├── Process = PnP
│   │   └── Operations:
│   │       ├── 1
│   │       │   └── Tasks
│   │       │       └── Load PCBs
│   │       ├── 2
│   │       │   └── Tasks
│   │       │       └── Place components
│   │       └── 3
│   │           └── Tasks
│   │               └── Reflow soldering
│   └── Process = Manual
│       └── Operations:
│           ├── 1
│           │   └── Tasks
│           │       └── Load PCBs
│           └── 2
│               └── Tasks
│                   ├── Place components
│                   └── Manually solder components
├── PCBs
|   └── 1 (Panel)
|       ├── Unit 1 (x=1, y=1) - Design A (Variant A) 
|       ├── Unit 2 (x=2, y=1) - Design A (Variant A)
|       ├── Unit 3 (x=3, y=1) - Design A (Variant A)
|       └── Unit 4 (x=4, y=1) - Design A (Variant A)
└── Phases
    ├── 1 - Top (Surface Mount / SMT)
    │   ├── Process = PnP
    │   ├── Ordering: Panel Unit:Ascending, Feeder Reference:Ascending, Part Cost:Ascending, Part Area:Ascending, Quantity:Ascending
    │   └── Loadout: 'pnp-machine-1'.
    └── 2 - Top (Through-hole / TH)
        ├── Process = Manual
        ├── Ordering: Panel Unit:Ascending, Part Cost:Ascending, Part Area:Ascending, Quantity:Desc
        └── Loadout: 'desk-top'.
```

## PlannerGUI

The planner GUI is used to define and manage the PCB assembly process, it works for automated, manual and mixed process
assembly jobs.

It's primary function is to allow you to define the assembly job and manage the job progress. It also generates artifacts
required for each process (e.g. a load-out and placement files).

Here's a recent screenshot:

[<img src="assets/screenshots/plannergui/planner_gui_2025-04-22_132254.png" width="800" alt="PlannerGUI">](assets/screenshots/plannergui/planner_gui_2025-04-22_132254.png)

## PlannerCLI

The PlannerCLI has the same functionality as the PlannerGUI, but with a command line interface.

Its `parts` mode uses the stores core (`planning/stores_app`), which has the user's data sources, for example
`planner_cli parts --source parts.csv list` lists a parts file's parts as CSV, and
`planner_cli parts --source parts.csv add --manufacturer RESCO --mpn RES1 --value LCSC=C1234` adds a part, as `set`
sets a part's other values and `remove` removes one. A change is refused if the file has changed since it was read, or
since the revision given with `--revision`. The stores core is pure: the CLI has the stores service
(`planning/stores_service`) read and write the file for it, as the GUI does.

Here's the current help output:

```
$ ./target/debug/planner_cli.exe --help
Usage: planner_cli [OPTIONS] <--project <PROJECT_NAME>> <COMMAND>

Commands:
  create                          Create a new job
  add-pcb                         Add a PCB
  assign-variant-to-unit          Assign a design variant to a PCB unit
  assign-process-to-parts         Assign a process to parts
  create-phase                    Create a phase
  assign-placements-to-phase      Assign placements to a phase
  assign-feeder-to-load-out-item  Assign feeder to load-out item
  set-placement-ordering          Set placement ordering for a phase
  generate-artifacts              Generate artifacts
  record-phase-operation          Record phase operation
  record-placements-operation     Record placements operation
  reset-operations                Reset operations
  help                            Print this message or the help of the given subcommand(s)

Options:
      --trace [<TRACE>]         Trace log file
      --path <PATH>             Path [default: .]
      --project <PROJECT_NAME>  Project name
  -v, --verbose...              Increase logging verbosity
  -q, --quiet...                Decrease logging verbosity
  -h, --help                    Print help
  -V, --version                 Print version
```

## VariantBuilderCLI

The variant builder CLI is used to take output files from EDA tools (e.g. DipTrace, KiCad, EasyEDAPro) and build
normalized placement files for the Planner.  Normalizing consists of these main tasks:

1) Cross-referencing EDA tool BOM information with your part inventory, using rules.
2) Substituting parts, using rules.
3) Selecting suitable parts from potential candidates, using load-out and rules. 
4) Normalizing units and co-ordinate systems.

It's common in small-volume manufacturing to have just a single PnP machine with feeder load-out that you don't want
to change.  The Variant Builder can prefer to use parts that are already in the machine so that you have to change
as few feeders as possible.  This is particularly useful for users of machines with drag-pin feeders, such as the
CharmHigh CHMT32VA/CHMT32VB/CHMT48VA/CHMT48VB machines, where the feeders are not individually replaceable.  The 
variant builder software is sensitive to the knowledge that changing feeders and machine-setup can be a very
time-consuming process. 

Here's the current help output:

```
$ ./target/debug/variantbuilder_cli.exe build --help
Build variant

Usage: variantbuilder_cli build [OPTIONS] --eda <EDA> --placements <SOURCE> --parts <SOURCE> --part-mappings <SOURCE> --output <FILE>

Options:
      --eda <EDA>
          EDA tool [possible values: diptrace, kicad, easyeda]
      --load-out <SOURCE>
          Load-out source
      --placements <SOURCE>
          Placements source
  -v, --verbose...
          Increase logging verbosity
      --parts <SOURCE>
          Parts source
  -q, --quiet...
          Decrease logging verbosity
      --part-mappings <SOURCE>
          Part-mappings source
      --substitutions [<SOURCE>...]
          Substitution sources
      --ref-des-disable-list [<REF_DES_DISABLE_LIST>...]
          List of reference designators to disable (use for do-not-fit, no-place, test-points, fiducials, etc)
      --assembly-rules <SOURCE>
          Assembly rules source
      --output <FILE>
          Output CSV file
      --name <NAME>
          Name of assembly variant [default: Default]
      --ref-des-list [<REF_DES_LIST>...]
          List of reference designators
  -h, --help
          Print help
```

## Gerber Viewer

There's a stand-alone gerber viewer, it can render gerber files generated with DipTrace 4.3, KiCad 8.0, and other tools.

Here's a screenshot:

[<img src="assets/screenshots/gerber_viewer/gerber_viewer_2025-05-01_221636.png" width="800" alt="GerberViewer">](assets/screenshots/gerber_viewer/gerber_viewer_2025-05-01_221636.png)

### Supported gerber features

Full details of supported gerber features are in the gerber-viewer library crate, here:

https://github.com/MakerPnP/gerber-viewer#supported-gerber-features

### Feedback 

If you have gerber files and have rendering issues, please create an issue with screenshots and a gerber file.  If
you're able to create a gerber file with just the elements that have issues that would be preferred.


## Getting started

Currently, you have to build from source, but WAIT! it's EASY!

1) install rust (use [Rustup](https://www.rust-lang.org/tools/install))
2) clone the repo (use git via `git clone https://github.com/MakerPnP/makerpnp.git`, or [download a zip file from github](https://github.com/MakerPnP/makerpnp/archive/refs/heads/master.zip)) and extract it.
3) run the following commands in your terminal:
```
cd makerpnp
cargo build --release
```

After it's built there's executable files in the `target/release` directory:

```
$ ls -g target/release/*.exe
-rwxr-xr-x 2 None 13341184 Apr 28 14:06 target/release/gerber_viewer_egui.exe
-rwxr-xr-x 2 None  5775872 Apr 28 14:06 target/release/planner_cli.exe
-rwxr-xr-x 2 None 22119936 Apr 28 14:06 target/release/planner_gui_egui.exe
-rwxr-xr-x 2 None  3703296 Apr 28 14:06 target/release/variantbuilder_cli.exe
```

## Running the tools

The CLI tools can be run from anywhere, they are self-contained.
The GUI tools need to be run from the corresponding source directory as they require assets.  e.g.

```
cd planning/planner_gui_egui
../../target/release/planner_gui_egui.exe
```

## Architecture

The current architecture is to use Crux to provide rust-based back-ends (cores), and then to write Crux
'shells', also in rust, to provide front-ends (GUI/CLI) or APIs (e.g. web/rpc/etc) which use the common cores.  This
does add a layer of complexity, but should future-proof the project somewhat and also allows creation of non-rust
native apps that use the rust core.  Check out the 'Crux' documentation.

Given the state of Rust GUI frameworks, and that a 3 month-long investigation was done and documented in the form of a 
40+ livestream series. Using Crux gives the project a way of being able to change the GUI library for a different one
if the chosen GUI library, egui, does not meet future requirements.  This approach turned out to be very useful when
the Cushy GUI framework was replaced with egui.

See:
* Comparison of GUI libraries - https://docs.google.com/spreadsheets/d/1cxH_GgzrGDpXm4CN0cWvQ9RF_PLf6HEvVYcZhqOO0nc/edit?usp=sharing
* 'Trying Rust GUI tech in 2024' video series playlist - https://www.youtube.com/playlist?list=PLUCLWCDEWm8g7pHKQGE7Pokk4wiVU8rLl 

In the end, [egui](https://github.com/emilk/egui) was chosen for the GUI framework due to it's large community and
ability to create usable productivity-style GUIs.

Currently, it's thought that the software wil have these main components:
* Machine control - for co-ordination of hardware control through one or more drivers (gcode, cameras, vision).
* Machine operator UI - for machine configuration, job setup, and job operations.  feeders, cameras, head/nozzle/tool
  control, integrates with the machine control and planner components.  
* Planner UI - used to manage assembly projects, can be used without a pick-and-place machine, can generate artifacts
  usable by the operator UI (jobs), with EDA support (KiCad, Diptrace, EasyEda, etc.).

If you're familiar with 3D printing, you can think of the Planner as the 'slicer', the operator UI as the 3D printer's
front panel (touchscreen/buttons/display), and Machine control is a bit like the firmware, but not, since the machine
control component will interact with one or more drivers which will communicate to one or more pieces of hardware, each
with their own firmware, where as 3d printers usually only have one main control-board with a single piece of firmware. 

Examples of hardware which *could* be interacted with include:
* dedicated pink-and-place boards, usually via gcode (for x/y/z, vacuum, nozzle control, feeder actuators)
* 3d printer control boards, usually via gcode (again for x/y/z, vacuum, nozzle control, feeder actuators)
* feeder actuator systems (e.g. via RS485/RS232 + arbitrary protocol)
* conveyor/stacker systems (for loading pcbs into a machine)
* local or ip cameras
* and anything else you can dream of.

## Documentation

Written documentation for the project is sparse, further documentation will be created in due course.  However, the 
design and operation of the tools is detailed in the livestreams.

There are also functional tests which detail the expected input/output and operation sequences, so you can refer to
those to gain additional understanding.  See `planning/planner_cli/tests/planner.rs` and
`planning/variantbuilder_cli/tests/variantbuilder.rs`.

## Links

Please subscribe to be notified of live-stream events so you can follow the development process.

* Patreon: https://www.patreon.com/MakerPnP
* Source: https://github.com/MakerPnP
* Discord: https://discord.gg/ffwj5rKZuf
* YouTube: https://www.youtube.com/@MakerPnP
* X/Twitter: https://x.com/MakerPicknPlace

## Authors

* Dominic Clifton - Project founder and primary maintainer.

## License

TBD (Probably GPL3, Apache or MIT)

## Contributing

If you'd like to contribute, please raise an issue or a PR on the github issue tracker, work-in-progress PRs are fine
to let us know you're working on something, and/or visit the discord server.  See the ![Links](#links) section above.