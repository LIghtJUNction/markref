---
name: markref
description: Locate shared features across languages using @mark source comments, retrieve bounded code context, and maintain useful navigation marks. Use for code navigation or requested source-label maintenance; fall back to rg when marks are absent.
license: AUL-1.0-draft.1; see LICENSE.txt in the markref distribution.
---

# markref

Use `markref` to reuse feature locations recorded beside source code. The CLI
is read-only; adding or changing comments is a separate source edit within the
user's requested scope. Do not add marks merely because this skill was loaded.

## Find and inspect

Check `markref --version` and use `markref --help` when flags are unfamiliar.
If the executable is missing, report that fact and use `rg`; install only when
installation is within the user's request. The registry install is
`cargo install markref --locked`.

Choose the intended Git worktree explicitly when cwd is ambiguous:

```sh
markref --root /path/to/project list
markref --root /path/to/project list --json
markref --root /path/to/project find "refund"
markref --root /path/to/project show payments.refund --json
markref --root /path/to/project context payments.refund --max-bytes 8000
markref --root /path/to/project context payments.refund --expand --max-bytes 8000
markref --root /path/to/project context payments.refund --json --max-bytes 8000
```

If the label is unknown, start with `list`: it returns sorted feature labels,
location counts and unique roles. It includes only scanned marks, not a list of
all implemented features. Then use a compact context map. Expand only the relevant feature, then read
its actual implementation, callers and tests. Byte limits are not token limits.
Use JSON context's `truncated` flag to distinguish a complete result from a
bounded excerpt; even a nontruncated result covers only scanned marks.

`find` searches mark metadata and adjacent comments, not every source line.
Summaries join up to eight adjacent comment lines, bounded to 180 characters;
blank lines separate blocks. Navigation commands warn on stderr if the scan
found diagnostics; JSON stdout remains machine-readable. Use `check --json`
to inspect the complete diagnostic list before treating results as complete.
No match exits nonzero; it does not prove the feature is absent. Fall back to
`rg --files` for paths and `rg -n` for errors, routes, config keys and symbols.
`--root` selects the containing Git worktree, not a subtree filter.

## Add or maintain marks when requested

Use the local language's real comment syntax. Keep one stable feature label
across components, with optional `role` and `contract`:

```rust
// Handles refunds against the original payment.
// @mark payments.refund role=authority
fn refund() {}
```

```python
# @mark payments.refund role=tool
def inspect_refund():
    pass
```

```sql
-- @mark payments.refund role=schema
CREATE TABLE refund_receipts (id bigint PRIMARY KEY);
```

Place marks beside existing documentation and code. Labels use ASCII letters,
digits, underscores, hyphens and dots, without empty segments; each segment
starts with a letter or digit. Fields are whitespace-separated `key=value`
tokens without spaces in values. Only `role` and `contract` are supported in
the current version; aliases, summaries and explicit symbol fields are not
supported syntax.
Keep descriptions in adjacent ordinary comments instead.

Reuse labels freely: authority, caller, UI and tests can all share a feature.
Avoid tagging every helper. Shared labels express related functionality; shared
contracts express declared interfaces, not verified connections or successful
calls. Verify real protocol and business behavior separately.

After comment edits, run `markref --root /path/to/project check` and inspect the
target symbols. Standalone marks attach to following code within 24 lines;
inline marks attach to preceding code on the same line. Symbol detection and
the comment lexer are heuristic, so inspect suspicious bindings manually.
Do not rewrite unrelated source just to eliminate existing diagnostics.

## Scope and limitations

Default scans include only Git-tracked recognized source files. New unstaged
files need explicit `--include-untracked`; this flag still honors Git ignores.
Do not stage files merely to make a scan see them. Submodules and symlinks are
not followed; scan a submodule explicitly only when it is in scope. Binary,
oversized and unsupported files are skipped. `check --json` exposes diagnostics;
read warnings and errors instead of treating a zero exit as full coverage.

There is no persistent index, daemon or automatic source rewriting. Do not
invent `index`, `add`, `rename` or automatic call-graph commands.
