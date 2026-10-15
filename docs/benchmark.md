# Benchmark plan

This document defines the evaluation method. It has no results yet.

## Search performance

Use at least three Git repositories: small (under 100 tracked source files),
medium (1,000–10,000), and large (over 50,000 where possible).
Record commit SHA, CPU, OS, Rust version, total tracked bytes,
and mark density.

Compare:
- `git grep -n '@mark'` as a tracked-source search baseline.
- `rg -n '@mark'` as a general text search baseline (account for scope).
- `markref show LABEL`, `find TEXT`, and `context LABEL`.

Use equal file scopes and define which output fields each command supplies.
Collect 20 cold and 50 warm runs and report median, p95, peak RSS,
and precision/recall against a reviewed list of expected locations.
Include source edits, branch changes, and .gitignore updates.
Do not call results equivalent if one command performs extra validation.

## Agent workflow experiment

Create paired tasks: locate a feature entry, trace an RPC contract,
find an auth policy, and identify related tests across languages.
Use a fixed model, repository commit, prompt, token price and
acceptance tests. Randomize ordering and measure:
- correct completed patches
- input/output tokens and tool calls
- wall time and source bytes read
- wrong locations and mistaken assumptions about linked code

Compare regular search against markref-assisted search.
Report failures and sample sizes. Remove secrets from traces.

## Important limitations

The v0.1 comment lexer is not a full compiler parser.
Include raw strings, nested comments, Unicode, renamed files,
new/ignored files, and unsupported languages in the correctness suite.
A faster result is not a benefit when correctness falls.
