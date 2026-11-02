# Planner — Known issues

Problems that are known and not yet fixed, to come back to. When one is decided, its outcome becomes a requirement in the
relevant specification, and it is removed from here.

| ID | Issue | Notes |
|---|---|---|
| `ISSUE-1` | Creating a project or PCB overwrites any existing file of the same name, without warning, as soon as Ok is clicked (`DOC-1`). This loses the existing document. The CLI's `create` commands do the same. | A suggested fix is for the core to refuse to create a document whose file already exists, and for the form to show the error. Not decided. |
| `ISSUE-2` | A PCB in a subfolder of the project is stored with the separator of the system that added it (`PROJPCB-2`), so a project made on Windows may not find that PCB on Linux. | From reading the code, not tested. A PCB directly in the project's folder has no separator, so isn't affected. |
| `ISSUE-3` | The minimum height of a table (`FORM-8`) assumes the default row height of 18 and spacing of 3 (`forms/mod.rs`, `TABLE_ROW_HEIGHT`), so the minimum is wrong if the rows are a different height. | This needs to be dynamic, set at runtime from the egui style in use, likely the body text size, `interact_size` and `item_spacing`. |

IDs are never reused.
