# ISSUE-22 — A file given to the CLI as `absolute=` keeps the prefix in its path

| | |
|---|---|
| Status | Open |
| Tool | Planner: CLI |
| Requirements | `PROJPCB-2`, `PROJPROC-1` |
| Found | 2026-11-04 |

## Description

A file given to the CLI as `absolute=<path>` is read and stored with `absolute=` as part of its path, so it can't be
found.

## Steps to repeat

In a folder with a project `job1` and a process file `pnp.process.json`:

1. `planner_cli project --project job1 add-process --file absolute=<the absolute path of pnp.process.json>`

**Expected:** the process is added, with its absolute path.

**Actual:** the CLI tries to load `absolute=<path>`, and fails. On Windows the error is
`IoError(Os { code: 123, kind: InvalidFilename, ... })`.

`add-pcb` reads its file the same way.

## Last repeated

2026-11-04, Windows, after `6f983ed0`: reproduced.

## Notes

`FileReference::try_from` in `planning/src/file.rs` builds the path from the whole value, rather than from what follows
`=`.

## Workaround

Put the file in, or below, the project's folder, and use `relative=<path>`.
