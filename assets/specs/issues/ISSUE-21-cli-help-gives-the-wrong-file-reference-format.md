# ISSUE-21 — The CLI's help gives the wrong format for a file reference

| | |
|---|---|
| Status | Open |
| Tool | Planner: CLI |
| Requirements | `PROJPCB-2` |
| Found | 2026-11-04 |

## Description

The help for `project add-pcb --file`, and `PROJPCB-2`, say a relative path is given as `relative:<path>`, but the CLI
only accepts `relative=<path>`. The help for `add-pcb` and `add-process` also says a bare absolute path can be given,
but the CLI only accepts `relative=` or `absolute=` (see `ISSUE-22`).

## Steps to repeat

In a folder with a project `job1` and a process file `pnp.process.json`:

1. `planner_cli project --project job1 add-pcb --help`, and the same for `add-process`.
2. `planner_cli project --project job1 add-process --file relative:pnp.process.json`
3. `planner_cli project --project job1 add-process --file <the absolute path of pnp.process.json>`

**Expected:** each form the help gives is accepted.

**Actual:** step 1's help gives `relative:<path>` for `add-pcb`, and a bare absolute path for both. Steps 2 and 3 fail
with "Invalid file reference format. Required format is 'relative=path' or 'absolute=path'."

## Last repeated

2026-11-04, Windows, after `6f983ed0`: reproduced.

## Notes

The help is in `opts.rs`. The parsing is `FileReferenceParser` in `common/cli/src/parsers.rs`, and
`FileReference::try_from` in `planning/src/file.rs`.

## Workaround

Use `relative=<path>`.
