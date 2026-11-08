theme-button-light = ☀ Light
theme-button-dark = 🌙 Dark
theme-button-system = 💻 System

# format "<language in native language> (<country in native language>)
language-es-ES = Español (España)
language-en-US = English (United States)

menu-top-level-file = File
menu-item-quit = Quit

modal-errors-title = Errors - { $file }
modal-add-phase-title = Add phase - { $file }
modal-package-sources-title = Package sources - { $file }
modal-create-unit-assignment-title = Create unit assignment - { $file }
modal-phase-placement-orderings-title = Phase placement orderings - { $phase }
modal-manager-gerbers-title = Manage gerbers - { $design }

toolbar-button-home = Home
toolbar-button-new-project = New project
toolbar-button-open-project = Open project
toolbar-button-new-pcb = New PCB
toolbar-button-open-pcb = Open PCB
toolbar-button-new-process = New process
toolbar-button-open-process = Open process
toolbar-button-open-parts = Open parts
toolbar-button-new-design-variant = New design variant
toolbar-button-open-design-variant = Open design variant
toolbar-button-save = Save
toolbar-button-close-all = Close all

project-toolbar-button-show-explorer = Show explorer
project-toolbar-button-generate-artifacts = Generate artifacts
project-toolbar-button-refresh = Refresh
project-toolbar-button-remove-unused-placements = Remove unused placements
project-toolbar-button-add-pcb = Add PCB
project-toolbar-button-add-process = Add process
project-toolbar-button-add-phase = Add phase
project-toolbar-button-package-sources = Package sources
project-toolbar-button-reset-operations = Reset operations

project-pcb-toolbar-button-create-unit-assignment = Create unit assignment
project-pcb-toolbar-button-show-pcb = Show PCB

tab-label-home = Home
tab-label-new-project = New project
tab-label-new-pcb = New PCB
tab-label-new-process = New process
tab-label-open-parts = Open parts
tab-label-new-design-variant = New design variant

home-banner = MakerPnP - Planner
home-checkbox-label-show-on-startup = Show on startup

new-project-banner = New project
form-new-project-input-name = Project name
form-new-project-input-directory = Directory

project-detail-path = Path: { $path }
project-detail-name = Name: { $name }

project-issues-tab-label = Issues

project-overview-tab-label = Overview
project-overview-detail-name = Name: { $name }
project-overview-phases-header = Phases
project-overview-pcbs-header = PCBs
project-overview-pcbs-input-remove-disabled-hover-text-in-use-1 = In use - remove PCB unit assignments.

project-pcb-designs-header = Designs

project-placements-tab-label = Placements
project-placements-tab-phase-hover-text-no-phases = No phases defined.
project-placements-tab-phase-hover-text-no-selection = No selection.
project-placements-tab-phase-hover-text-with-selection = Phase to apply to selection.
project-placements-tab-skipped-other-side = { $count ->
    [one] 1 placement was not changed, it's on the other side of the PCB to the phase.
   *[other] { $count } placements were not changed, they're on the other side of the PCB to the phase.
}

project-parts-tab-label = Parts

project-pcb-tab-label = PCB ({ $pcb })

project-process-tab-label = Process ({ $process })
project-process-tab-button-open = Open file
project-process-tab-button-refresh = Refresh processes
project-process-tab-button-remove = Remove process
project-process-tab-detail-file = File: { $file }
project-process-tab-detail-reference = Reference: { $reference }
project-process-tab-task = { $task } ({ $kind })

project-unit-assignments-tab-label = Unit assignments ({ $pcb })

phase-toolbar-add-parts-to-loadout = Add parts to load-out.
phase-toolbar-placement-orderings = Placement orderings.

phase-reference = Reference
phase-load-out-source = Load-out source
phase-pcb-side = PCB Side
phase-process = Process

table-load-out-column-reference = Reference
table-load-out-column-manufacturer = Manufacturer
table-load-out-column-mpn = MPN

pcb-side-top = Top
pcb-side-bottom = Bottom
pcb-side-both = Both

project-explorer-tab-label = Project explorer

project-explorer-node-root = Overview
project-explorer-node-issues = Issues
project-explorer-node-parts = Parts
project-explorer-node-placements = Placements
project-explorer-node-phases = Phases
project-explorer-node-phase = { $reference } ({ $process } - { $pcb_side })
project-explorer-node-phase-loadout = { $source }
project-explorer-node-unit-assignments = Unit assignments
project-explorer-node-unit-assignment-assigned = { $name } {$design_name} = {$variant_name}
project-explorer-node-unit-assignment-unassigned = { $name } {$design_name} = Unassigned
project-explorer-node-pcbs = PCBs
project-explorer-node-pcb = { $name }
project-explorer-node-processes = Processes
project-explorer-node-process = { $name }

new-pcb-banner = New PCB
form-new-pcb-input-name = PCB name
form-new-pcb-input-name-placeholder = (e.g. the PCB factory's order reference number)
form-new-pcb-input-directory = Directory
form-new-pcb-input-units = Units

new-process-banner = New process
form-new-process-input-reference-placeholder = (e.g. 'glue_and_wave_solder')
form-new-process-input-directory = Directory
form-new-process-input-preset = Preset

process-detail-path = Path: { $path }

form-open-parts-input-source = Data source
form-open-parts-source-none = --choose source--
form-open-parts-source-csv-file = CSV file
form-open-parts-group-csv-file = CSV file
form-open-parts-input-csv-file = File

parts-source-csv-file = CSV file: { $path }
parts-source-button-reload = Reload
parts-source-no-parts = There are no parts.
parts-source-loading = Loading the parts...
parts-source-input-manufacturer = Manufacturer
parts-source-input-mpn = MPN
parts-source-button-add-part = Add part
parts-source-button-remove-part = Remove part

variant-builder-path = Design variant: { $path }
variant-builder-loading = Loading the design variant...
variant-builder-input-eda-tool = EDA tool
variant-builder-input-placements = Placements
variant-builder-input-design-name = Design name
variant-builder-input-variant-name = Variant name
variant-builder-filter-placements = Placements
variant-builder-filter-csv = CSV files
variant-builder-group-part-matching = Part matching
variant-builder-input-parts = Parts
variant-builder-input-part-mappings = Part mappings
variant-builder-input-global-substitutions = Global substitutions
variant-builder-group-design-specific = Design specific
variant-builder-input-assembly-rules = Assembly rules
variant-builder-input-design-substitutions = Design substitutions
variant-builder-group-reference-designators = Reference designators
variant-builder-input-ref-des-list = Ref-des list
variant-builder-input-ref-des-exclusion-list = Exclusion list
variant-builder-input-ref-des-no-place-list = No-place list
variant-builder-group-output = Output
variant-builder-input-output-placements = Output placements
variant-builder-input-output-bom = Output BOM
variant-builder-output-default = (named after the design variant)
variant-builder-button-add = Add
variant-builder-button-remove = Remove
variant-builder-button-move-up = Move up
variant-builder-button-move-down = Move down
variant-builder-button-new = New
variant-builder-substitutions-applied = Substitutions, in the order they're applied
variant-builder-substitutions-applied-none = (none)
variant-builder-column-order = Order
variant-builder-column-file = File
variant-builder-column-actions = Actions
variant-builder-column-source = Source
variant-builder-source-design = Design
variant-builder-source-global = Global
variant-builder-button-save = Save changes
variant-builder-button-build = Build
variant-builder-built = Built.
variant-builder-panel-explorer = Explorer
variant-builder-panel-overview = Overview
variant-builder-panel-build-output = Build output
variant-builder-panel-output-placements = Placements
variant-builder-panel-output-bom = BOM
variant-builder-explorer-none = (none)
variant-builder-build-output-to-come = The mapping results are to come.
variant-builder-data-source-path = File: { $path }
variant-builder-no-eda-tool = Choose the EDA tool to show the placements.
data-table-button-add-row = Add row
data-table-button-remove-row = Remove row
data-table-button-move-up = Move up
data-table-button-move-down = Move down
data-table-pattern-matched = Matched by { $tool }.
data-table-pattern-not-matched = Not matched by { $tool }.
data-table-pattern-no-patterns = Matched by { $tool }. None of its fields has a pattern.
data-table-value-substituted = Used to substitute { $tool } values.
data-table-value-not-substituted = Not used to substitute { $tool } values.
data-table-value-no-values = Used to substitute { $tool } values. Substitution values not defined.
data-table-status-no-problems = No problems.
data-table-problem-invalid-pattern = A pattern isn't valid.
data-table-problem-missing-field-columns = The data source has no column for one of { $tool }'s fields.
data-table-problem-no-patterns = None of { $tool }'s fields has a pattern.
data-table-problem-missing-part-columns = The data source has no manufacturer's column, or no MPN's.
data-table-problem-empty-part = The manufacturer, or the MPN, is empty.
data-table-problem-no-values = Substitution values not defined.
data-table-problem-changes-nothing = Matching patterns and values have no effect.
table-parts-source-column-manufacturer = Manufacturer
table-parts-source-column-mpn = MPN
process-editor-operation = Operation
process-editor-task = task
process-editor-button-move-up = Move up
process-editor-button-move-down = Move down
process-editor-input-task-kind = Kind
process-editor-input-new-task = New task
process-editor-input-new-task-placeholder = (e.g. 'apply_glue')
process-editor-button-add-task = Add task
process-editor-input-new-operation = New operation
process-editor-button-add-operation = Add operation
process-editor-rules = Rules
process-editor-error-operation-reference-invalid = Operation { $operation }: reference invalid
process-editor-error-task-reference-invalid = Operation { $operation }, task { $number }: reference invalid

process-task-kind-generic = Generic
process-task-kind-load-pcbs = Load PCBs
process-task-kind-place-components = Place components

pcb-configuration-tab-label = Configuration
pcb-configuration-detail-name = Name: { $name }

pcb-panel-tab-label = Panelization
pcb-panel-tab-panel-orientation-header = Assembly orientation
pcb-panel-tab-panel-size-header = Panel size
pcb-panel-tab-panel-edge-rails-header = Edge rails
pcb-panel-tab-panel-fiducials-header = Fiducials
pcb-panel-tab-panel-design-configuration-header = Design configuration
pcb-panel-tab-panel-unit-positions-header = Unit positions

pcb-assembly-orientation-rotation = Rotation
pcb-assembly-orientation-flip-tooltip = Terminology:
    Roll = hold pcb by top and bottom and rotate 180 degrees along the Y-axis (mirrors X coordinates).
    Pitch = hold pcb by left and right and rotate 180 degrees along the X-axis (mirrors Y coordinates).
pcb-assembly-orientation-flip-roll = Roll (X mirroring)
pcb-assembly-orientation-flip-pitch = Pitch (Y mirroring)
pcb-assembly-orientation-flip-none = None

pcb-explorer-tab-label = PCB Explorer
pcb-explorer-node-root = { $name }
pcb-explorer-node-configuration = Configuration
pcb-explorer-node-panel = Panelization
pcb-explorer-node-pcb-view = PCB
pcb-explorer-node-designs = Designs
pcb-explorer-node-units = Unit
pcb-explorer-node-units-assignment-assigned = { $pcb_number }: {$design_name}
pcb-explorer-node-units-assignment-unassigned = { $pcb_number }: Unassigned

pcb-gerber-viewer-tab-label-panel = Panel
pcb-gerber-viewer-tab-label-design = Design ({ $index })
pcb-gerber-viewer-layers-window-title = Layers
pcb-gerber-viewer-input-go-to = Go to

form-configure-pcb-input-units = Units
form-configure-pcb-input-gerber-offset = Gerber offset
form-configure-pcb-group-unit-map = Unit mapping
form-configure-pcb-input-design-name = Design name
form-configure-pcb-input-design-name-placeholder = (e.g. 'my_eda_project', for unit assignments)
form-configure-pcb-input-pcb-unit-range = Unit range
form-configure-pcb-button-panel-gerbers = Panel Gerbers...
form-configure-pcb-button-design-gerbers = Design Gerbers...

form-configure-pcb-gerber-offset-help = In EDA software, an offset can be specified when exporting gerbers, e.g. (10,5).
    Enter negative offsets here to relocate the gerbers back to (0,0), e.g. (-10, -5)

form-create-unit-assignment-group-variant-map = Variant map
form-create-unit-assignment-input-design-name = Design name
form-create-unit-assignment-input-design-name-placeholder = Design name (e.g. 'my design')
form-create-unit-assignment-input-variant-name = Variant name
form-create-unit-assignment-input-variant-name-placeholder = Variant name (e.g. 'default')
form-create-unit-assignment-input-pcb-instance = PCB instance
form-create-unit-assignment-input-pcb-instance-placeholder = A number > 0
form-create-unit-assignment-input-pcb-unit-range = PCB units
form-create-unit-assignment-input-pcb-unit-placeholder = A number > 0
form-create-unit-assignment-input-placements-filename = Placements filename
form-create-unit-assignment-input-placements-filename-placeholder = '<design>_<variant>_placements.csv'
form-create-unit-assignment-input-placements-directory = Placements directory

form-phase-placement-orderings-input-orderings = Orderings

form-process-operations = Operations

form-package-sources-input-packages-source = Packages
form-package-sources-input-package-mappings-source = Package mappings

form-common-combo-select = Select...
form-common-combo-none = None

form-common-choice-pcb-kind = PCB Kind
form-common-choice-pcb-kind-single = Single
form-common-choice-pcb-kind-panel = Panel

form-common-choice-pcb-side = PCB Side
form-common-choice-pcb-side-top = Top
form-common-choice-pcb-side-bottom = Bottom

form-common-choice-process = Process
form-common-choice-phase = Phase

form-common-input-load-out-source = Loadout source
form-common-input-phase-reference = Phase reference
form-common-input-process-reference = Process reference
form-common-input-x = X
form-common-input-y = Y
form-common-input-top = Top
form-common-input-bottom = Bottom
form-common-input-left = Left
form-common-input-right = Right
form-common-input-start = Start
form-common-input-end = End
form-common-input-mask-diameter = Mask ⌀
form-common-input-copper-diameter = Copper ⌀

form-common-button-assign-selected = Assign selected
form-common-button-unassign-selected = Unassign selected
form-common-button-unassign-all = Unassign all
form-common-button-unassign-from-range = Unassign from range
form-common-button-unassign-range = Unassign range

form-common-button-apply-range = Apply range
form-common-button-apply-all = Apply all

form-common-button-ok = Ok
form-common-button-cancel = Cancel
form-common-button-close = Close
form-common-button-add = Add
form-common-resize = Resize
form-common-button-clear = Clear
form-common-button-choose = Choose
form-common-button-remove = Remove
form-common-button-apply = Apply
form-common-button-reset = Reset
form-common-button-refresh = Refresh
form-common-button-delete = Delete

form-option-error-required = * Required

form-input-error-empty = Cannot be empty
form-input-error-length = Minimum length { $min }
form-input-error-range = Out of range, required range: { $min } - { $max } (inclusive)
form-choice-empty = Choose an option

form-input-number-require-greater-than-zero = Require a number greater than zero
form-input-number-require-positive-number = Require a number
form-file-not-found = File not found

form-input-error-map-incorrect-entry-count = Incorrect entry count.  Required: { $required }, Actual: { $actual }
form-input-error-map-unassigned-entries = Mapping contains unassigned entries.

form-input-error-reference-invalid = Reference invalid
form-input-error-process-reference-invalid = Process reference invalid
form-input-error-loadout-source-invalid = Loadout-source invalid

assignment-assigned = Assigned
assignment-unassigned = Unassigned

placement-placed = Placed
placement-pending = Pending
placement-skipped = Skipped

placement-place = Place
placement-no-place = No-place

placement-project-status-used = Used
placement-project-status-unused = Unused

sort-mode-area = Area
sort-mode-feeder-reference = Feeder reference
sort-mode-height = Height
sort-mode-part = Part
sort-mode-pcb = PCB instance
sort-mode-pcb-unit = PCB unit
sort-mode-pcb-unit-xy = PCB unit X, then Y
sort-mode-pcb-unit-yx = PCB unit Y, then X
sort-mode-ref-des = Ref. Des.

sort-order-ascending = Ascending
sort-order-descending = Descending

process-status-pending = Pending
process-status-incomplete = Incomplete
process-status-complete = Complete
process-status-abandoned = Abandoned
process-task-action-start = Start
process-task-action-complete = Complete
process-task-action-abandon = Abandon

gerber-file-function-assembly = Assembly
gerber-file-function-component = Component
gerber-file-function-copper = Copper
gerber-file-function-legend = Legend
gerber-file-function-paste = Paste
gerber-file-function-profile = Profile
gerber-file-function-other = Other
gerber-file-function-solder = Solder

table-placements-column-index = #
table-placements-column-object-path = Object path
table-placements-column-refdes = Ref. Des.
table-placements-column-place = Place
table-placements-column-placed = Placed?
table-placements-column-manufacturer = Manufacturer
table-placements-column-mpn = MPN
table-placements-column-rotation = Rotation
table-placements-column-x = X
table-placements-column-y = Y
table-placements-column-pcb-side = Side
table-placements-column-phase = Phase
table-placements-column-status = Status
table-placements-column-ordering = Ordering

table-parts-column-index = #
table-parts-column-manufacturer = Manufacturer
table-parts-column-mpn = MPN
table-parts-column-processes = Processes
table-parts-column-ref-des-set = Ref. Des. Set
table-parts-column-quantity = Quantity

table-designs-column-index = #
table-designs-column-name = Name
table-designs-column-actions = Actions

table-gerbers-column-index = #
table-gerbers-column-file = File
table-gerbers-column-pcb-side = PCB Side
table-gerbers-column-gerber-file-function = Purpose
table-gerbers-column-actions = Actions

table-gerber-viewer-layers-column-index = #
table-gerber-viewer-layers-column-file = File
table-gerber-viewer-layers-column-pcb-side = PCB Side
table-gerber-viewer-layers-column-gerber-file-function = Purpose

table-design-assignments-column-pcb-unit = PCB Unit
table-design-assignments-column-design = Design Name

table-design-variants-column-design = Design name
table-design-variants-column-variant = Variant name

table-unit-assignments-column-pcb-unit = PCB Unit
table-unit-assignments-column-design = Design name
table-unit-assignments-column-variant = Variant name

table-design-layout-column-index = #
table-design-layout-column-x-placement-offset = X placement offset
table-design-layout-column-y-placement-offset = Y placement offset
table-design-layout-column-x-gerber-offset = X gerber offset
table-design-layout-column-y-gerber-offset = Y gerber offset
table-design-layout-column-x-origin = X origin
table-design-layout-column-y-origin = Y origin
table-design-layout-column-x-size = X size
table-design-layout-column-y-size = Y size
table-design-layout-column-design-name = Design name

table-unit-positions-column-index = #
table-unit-positions-column-x = X
table-unit-positions-column-y = Y
table-unit-positions-column-rotation = Rotation
table-unit-positions-column-design-name = Design name

table-fiducials-column-index = #
table-fiducials-column-x = X
table-fiducials-column-y = Y
table-fiducials-column-mask-diameter = Mask ⌀
table-fiducials-column-copper-diameter = Copper ⌀
table-fiducials-column-actions = Actions

table-phases-column-index = #
table-phases-column-name = Name
table-phases-column-status = Status
table-phases-column-actions = Actions

table-pcbs-column-index = #
table-pcbs-column-file = File
table-pcbs-column-actions = Actions

table-issues-column-index = #
table-issues-column-severity = Severity
table-issues-column-message = Message
table-issues-column-details = Details
table-issues-column-actions = Actions

common-value-not-available = N/A
common-actions = Actions

common-value-operation-reference-default = operation_name

ratio = Ratio: { $numerator }: { $denominator }
ratio-error = Ratio: N/A

filter-expression = Search...

expanding-header-details = Details

#
# egui-data-tables
#

# cell context menu
context-menu-selection-copy = Selection: Copy
context-menu-selection-cut = Selection: Cut
context-menu-selection-clear = Selection: Clear
context-menu-selection-fill = Selection: Fill
context-menu-clipboard-paste = Clipboard: Paste
context-menu-clipboard-insert = Clipboard: Insert
context-menu-row-duplicate = Row: Duplicate
context-menu-row-delete = Row: Delete
context-menu-undo = Undo
context-menu-redo = Redo
# column header context menu
context-menu-hide = Hide column
context-menu-hidden = Hidden columns
context-menu-clear-sort = Clear sorting
