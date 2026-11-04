# ISSUE-23 — A parts file's values that look like numbers failed to load

| | |
|---|---|
| Status | Open: the stores core is fixed, but no test checks the log lines yet, see "What fixed means" |
| Tool | Variant Builder: core and CLI, and the stores core (`stores::parts`, used by both) |
| Requirements | `STORE-1` |
| Found | 2026-11-04 |
| Checked by | `planning/stores_app/tests/parts.rs`, `keeps_values_that_look_like_numbers` |

## Description

A parts CSV file couldn't be loaded if any value looked like a number, for example an MPN of `1234`, or a package of
`0402` in a column of its own. `variantbuilder_cli build` failed, as did anything else that loaded parts, with an error
that didn't say which value or why.

## Steps to repeat

Through the stores core, as `keeps_values_that_look_like_numbers` does:

1. Make `parts.csv`:

   ```
   "Manufacturer","Mpn","Package"
   "RESCO","1234","0402"
   ```

2. Send the stores core `LoadParts` with `parts.csv`, then `RequestPartsView`.

   Logs, in order:
   - `stores_app: Received event. event: LoadParts {`

**Expected:** the view has the part, with `1234` and `0402` as they are in the file. Logs, in order:
- `stores_app: Loaded parts. source: 'parts.csv', part_count: 1`

**Actual, before the fix:** the parts weren't loaded. Logs, in order:
- `stores_app: Event failed. error: UnableToLoadParts {`, with a cause of the form
  "`Deserializing part record: CSV deserialize error: record <n> (line: <n>, byte: <n>): invalid type: integer <value>, expected a string`"

## Last repeated

2026-11-04, by `keeps_values_that_look_like_numbers` and a first version of
`loads_the_parts_from_a_csv_file_in_order_with_their_other_values`, Windows 11, before the fix: reproduced, with
"`invalid type: integer `402`, expected a string`" for a value of `0402`, and after it: fixed. The test checks the
view, not the log lines. The steps through `variantbuilder_cli build` come from reading the code, and haven't been
followed.

## Notes

`PartRecord` read the other columns into a `#[serde(flatten)]` map. With `flatten`, serde reads every field of the
record, including `Mpn`, through `deserialize_any`, and the csv crate then guesses each value's type, so `1234` and
`0402` were read as integers, which can't be put into a string. The fix reads each record's values as text, by header,
and says which header is missing if `Manufacturer` or `Mpn` is.

The other CSV stores that use `flatten`, or a map of the whole record, may have the same problem. Part mappings already
have a test for fields containing integers.

## Workaround

None before the fix, other than avoiding such values.
