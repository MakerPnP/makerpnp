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

1. `planner_cli --trace planner.log -v project --project job1 add-process --file absolute=<the absolute path of pnp.process.json>`

**Expected:** the process is added, with its absolute path. Logs, in order:
- `planner_app: Received event. event: AddProcess { process_file: Absolute("<the absolute path>") }`, with no
  `absolute=`
- `planning::process: Loading process. path: <the absolute path>`
- `planning::project: Added process. process: 'pnp'`
- no `Event failed.`

**Actual:** the CLI fails, with exit code 1. Logs, in order:
- `planner_app: Received event. event: AddProcess { process_file: Absolute("absolute=<the absolute path>") }`
- `planning::process: Loading process. path: absolute=<the absolute path>`
- `planner_app: Event failed. error: IoError(`, on Windows
  `IoError(Os { code: 123, kind: InvalidFilename, message: "The filename, directory name, or volume label syntax is incorrect." })`

`add-pcb` reads its file the same way.

## Last repeated

2026-11-04, Windows, after `8a4ed2ae`, with `LOG-1` and `LOG-2` added: reproduced.

## Notes

The event's `Absolute("absolute=…")` shows that the CLI's parsing is at fault, not the core's loading.
`FileReference::try_from` in `planning/src/file.rs` builds the path from the whole value, rather than from what follows
`=`.

## Workaround

Put the file in, or below, the project's folder, and use `relative=<path>`.
