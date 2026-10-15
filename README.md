# markref

A read-only Rust CLI for shared feature marks in source comments.

[中文说明](README.zh-CN.md) · [AUL license](LICENSE.txt)

Source code is the only source of truth. A mark moves with the code.
Labels name **features**, not language or directory paths. One label can
appear at many places: an authority, caller, UI component, or test.

## Install

Requires Rust and Git.

~~~sh
cargo install markref --locked
# Or install this checkout:
cargo install --path . --locked
# Development:
git clone https://github.com/LIghtJUNction/markref
cd markref
cargo run -- --help
~~~


## Agent skill

The maintained skill is [skills/markref/SKILL.md](skills/markref/SKILL.md).
It covers bounded navigation, cross-language marks, real scan scope and
fallback to `rg`. Installing the CLI does not install an agent skill.

For a first Codex skill installation, run from this checkout (review an existing
`markref` skill before replacing it):

~~~sh
mkdir -p "${CODEX_HOME:-$HOME/.codex}/skills"
cp -R skills/markref "${CODEX_HOME:-$HOME/.codex}/skills/"
cp LICENSE.txt UPSTREAM.toml "${CODEX_HOME:-$HOME/.codex}/skills/markref/"
~~~

Reload the agent session to discover the new skill, then invoke `$markref` or
ask it to locate a feature using source marks. Other agents can load the same
`SKILL.md` according to their own skill-directory convention.

## Use existing source comments

~~~rust
// Handles refunds for the commerce service.
// @mark payments.refund role=authority
fn refund() {}

// @mark payments.refund role=caller contract=rpc:Commerce.Refund
fn request_refund() {}
~~~

The minimum syntax is `// @mark payments.refund`. All fields are
optional. Supported fields are `role` and `contract`. Fields are
space-separated `key=value` tokens without spaces in values.
Feature labels use ASCII letters, digits, underscores, hyphens, and
dots with no empty segments. Roles are user-defined.

A Python, Go, JavaScript, Rust, or SQL component can reuse the same label.
The same `contract` means both marks refer to a named contract; it does
**not** prove that calls have been connected or succeeded.

## Commands

~~~sh
markref list                                  # discover labels, counts and roles
markref list --json
markref find "refund"                         # free-text substring search
markref find "退款"                           # finds nearby comments
markref show payments.refund                   # every location for this label
markref check                                  # format and unbound mark errors
markref context payments.refund                # compact overview
markref context payments.refund --expand       # limited code excerpts
markref context payments.refund --max-bytes 8000
markref show payments.refund --json            # machine-readable output
markref context payments.refund --json         # JSON, strict size limit
markref --root /path/to/project check
markref --include-untracked find "refund"
~~~

Output includes path, marker line, nearby target line, probable symbol,
role, contract, and adjacent existing source comments. `find` searches
those fields case-insensitively. `show` uses exact label equality.
`list` aggregates repeated labels and sorts labels/roles deterministically.
Adjacent comment blocks are joined in source order, bounded to eight lines and
180 characters, instead of taking just the last line. Blank lines separate blocks.
Navigation commands warn on stderr when scan diagnostics exist; JSON stdout
remains clean. Run `check --json` to inspect all errors and warnings.

`check` returns a nonzero status when a mark is invalid or cannot bind
to code. Repeated labels are valid and are not errors. `context` stays
compact unless `--expand` is set; `--max-bytes` bounds stdout, not stderr diagnostics.

## Scanning behavior

By default, markref uses `git ls-files` and scans **only tracked files**
in the selected Git worktree. This avoids untracked build outputs and
dependency copies. Submodules are not followed. Add
`--include-untracked` to include untracked files, while continuing to
honor standard Git exclusions, including `.gitignore`. Symbolic links,
binary data, and files above 16 MiB are skipped by default. The size
limit can be adjusted with `--max-file-bytes`.

A fixed-byte search discards files without `@mark`. Only candidate
files pass through a lightweight comment lexer. Source files are read
in parallel. v0.1 has **no persistent index or background daemon**,
so old branch positions cannot be returned from a stale cache.

Supported comment styles: slash and block comments for common Rust,
Go, C/C++, JavaScript/TypeScript and similar files; hash comments for
Python, shell and configuration files; double-dash comments for SQL
and Lua; and HTML-style comments for HTML/XML/SVG. Only recognized
source extensions are scanned. The lexer is not a complete language
parser, and unusual string syntax can cause false matches.

A standalone mark attaches to following code within 24 lines.
An inline mark attaches to code before its comment.
Symbol detection is heuristic. Marks express related functionality,
not a verified call graph.

## Development

~~~sh
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
~~~

See the [benchmark plan](docs/benchmark.md). No measured time or token
improvement is claimed yet.

## License

[Agent Upstream License (AUL-1.0-draft.1)](LICENSE.txt), copyright
(c) 2026 LIghtJUNction. This is a custom draft, **not OSI-approved**.
See [UPSTREAM.toml](UPSTREAM.toml) and
[the AUL repository](https://github.com/LIghtJUNction/AUL).
