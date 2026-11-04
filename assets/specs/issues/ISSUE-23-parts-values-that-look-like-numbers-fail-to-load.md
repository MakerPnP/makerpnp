# ISSUE-23 — A parts file's values that look like numbers failed to load

| | |
|---|---|
| Status | Fixed on 2026-11-04, in `11fe63f1` (the stores core) and the commit adding the CLI's `parts` mode (its test) |
| Tool | Variant Builder: core and CLI, and the stores core (`stores::parts`, used by both) |
| Requirements | `STORE-1` |
| Found | 2026-11-04 |
| Checked by | `planning/planner_cli/tests/parts.rs`, `lists_the_parts_with_their_other_values`, and `planning/stores_app/tests/parts.rs`, `keeps_values_that_look_like_numbers` |

## Description

A parts CSV file couldn't be loaded if any value looked like a number, for example an MPN of `1234`, or a package of
`0402` in a column of its own. `variantbuilder_cli build` failed, as did anything else that loaded parts, with an error
that didn't say which value or why.

## Steps to repeat

From the CLI, in an empty folder:

1. Make `parts.csv`:

   ```
   "Manufacturer","Mpn","Package"
   "RESCO","1234","0402"
   ```

2. `planner_cli --trace planner.log -v parts --source parts.csv list`

   Logs, in order:
   - `stores_app: Received event. event: LoadParts {`

**Expected:** the part is listed, with `1234` and `0402` as they are in the file. Logs, in order:
- `stores_app: Loaded parts. source: 'parts.csv', part_count: 1`
- `stores_app: Received event. event: RequestPartsView {`
- no `Event failed.`

**Actual, before the fix:** the parts weren't loaded. Logs, in order:
- `stores_app: Event failed. error: UnableToLoadParts {`, with a cause of the form
  "`Deserializing part record: CSV deserialize error: record <n> (line: <n>, byte: <n>): invalid type: integer <value>, expected a string`"

## Last repeated

2026-11-04, CLI, Windows 11, after `11fe63f1`, with the `parts` mode: not reproduced, the part was listed with `1234`
and `0402`, and the expected log lines were logged, in order. Before the fix, the CLI had no `parts` mode, and the issue
was reproduced by `keeps_values_that_look_like_numbers` and a first version of
`loads_the_parts_from_a_csv_file_in_order_with_their_other_values`, through the stores core, with
"`invalid type: integer `402`, expected a string`" for a value of `0402`. The steps through `variantbuilder_cli build`
come from reading the code, and haven't been followed.

## Notes

`PartRecord` read the other columns into a `#[serde(flatten)]` map. With `flatten`, serde reads every field of the
record, including `Mpn`, through `deserialize_any`, and the csv crate then guesses each value's type, so `1234` and
`0402` were read as integers, which can't be put into a string. The fix reads each record's values as text, by header,
and says which header is missing if `Manufacturer` or `Mpn` is.

The other CSV stores that use `flatten`, or a map of the whole record, may have the same problem. Part mappings already
have a test for fields containing integers.

## Workaround

None before the fix, other than avoiding such values.
