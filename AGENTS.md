# Development instructions

This project uses the Agent Upstream License (AUL-1.0-draft.1).
Read LICENSE.txt and UPSTREAM.toml before changes. AUL is a draft custom
license, not an OSI-approved license. Preserve third-party license terms.
The original markref repository is its own upstream; do not open self-PRs
or self-issues merely to satisfy upstream duties.

markref is read-only. Never modify a user's source files from a scan command.
Repeated feature labels are valid. A shared label or contract does not prove
that components are connected or that a call took place.
Keep the default scan restricted to Git-tracked files. Untracked files must
require an explicit flag, and ignored files must remain excluded.

Run cargo fmt --all -- --check, cargo test --all-targets, and
cargo clippy --all-targets -- -D warnings for changes. Add focused tests
for syntax edge cases, Git ignore behavior, and output bounds.
Do not claim a benchmark improvement without measuring it.
