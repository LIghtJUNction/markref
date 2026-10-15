use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use memchr::memmem;
use rayon::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Mark {
    pub label: String,
    pub path: String,
    pub line: usize,
    pub target_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub path: String,
    pub line: usize,
    pub severity: &'static str,
    pub message: String,
}

pub struct Report {
    pub root: PathBuf,
    pub files_scanned: usize,
    pub marks: Vec<Mark>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Copy)]
enum Syntax {
    Slash,
    Hash,
    Dash,
    Html,
}

fn syntax_for(path: &str) -> Option<Syntax> {
    let path = Path::new(path);
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "dockerfile" | "makefile" | "justfile" | ".env.example"
    ) {
        return Some(Syntax::Hash);
    }
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "rs" | "go" | "c" | "cc" | "cpp" | "h" | "hpp" | "js" | "jsx" | "ts" | "tsx" | "java"
        | "kt" | "kts" | "swift" | "cs" | "dart" | "php" | "scala" | "zig" | "css" | "scss"
        | "less" | "m" | "mm" => Some(Syntax::Slash),
        "py" | "sh" | "bash" | "zsh" | "rb" | "pl" | "r" | "yml" | "yaml" | "toml" | "ini"
        | "conf" | "nix" | "tf" => Some(Syntax::Hash),
        "sql" | "lua" => Some(Syntax::Dash),
        "html" | "htm" | "xml" | "svg" => Some(Syntax::Html),
        _ => None,
    }
}

pub fn scan_project(
    start: &Path,
    include_untracked: bool,
    max_file_bytes: u64,
    include_snippets: bool,
) -> Result<Report> {
    let root_output = Command::new("git")
        .arg("-C")
        .arg(start)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to start git; install Git and try again")?;
    if !root_output.status.success() {
        bail!(
            "not a Git worktree: {} (use --root with a Git project)",
            start.display()
        );
    }
    let root = PathBuf::from(
        String::from_utf8(root_output.stdout)
            .context("Git returned a non-UTF-8 project path")?
            .trim_end_matches(['\r', '\n']),
    );

    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(&root)
        .args(["ls-files", "-z", "--cached", "--full-name"]);
    if include_untracked {
        cmd.args(["--others", "--exclude-standard"]);
    }
    cmd.arg("--");
    let output = cmd.output().context("failed to list Git files")?;
    if !output.status.success() {
        bail!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let mut filenames = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8(part.to_vec()).context("non-UTF-8 filename in Git index"))
        .collect::<Result<Vec<_>>>()?;
    filenames.sort_unstable();
    filenames.dedup();
    filenames.retain(|path| syntax_for(path).is_some());

    let mut scanned = filenames
        .par_iter()
        .map(|path| scan_file(&root, path, max_file_bytes, include_snippets))
        .collect::<Vec<_>>();
    let mut marks = Vec::new();
    let mut diagnostics = Vec::new();
    let mut files_scanned = 0;
    for result in scanned.drain(..) {
        if result.read_ok {
            files_scanned += 1;
        }
        marks.extend(result.marks);
        diagnostics.extend(result.diagnostics);
    }
    marks.sort_by(|a, b| (&a.path, a.line, &a.label).cmp(&(&b.path, b.line, &b.label)));
    diagnostics.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    Ok(Report {
        root,
        files_scanned,
        marks,
        diagnostics,
    })
}

#[derive(Default)]
struct FileResult {
    read_ok: bool,
    marks: Vec<Mark>,
    diagnostics: Vec<Diagnostic>,
}

fn diagnostic(
    path: &str,
    line: usize,
    severity: &'static str,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        path: path.to_owned(),
        line,
        severity,
        message: message.into(),
    }
}

fn scan_file(root: &Path, path: &str, limit: u64, include_snippets: bool) -> FileResult {
    let mut out = FileResult::default();
    let full_path = root.join(path);
    let metadata = match fs::symlink_metadata(&full_path) {
        Ok(m) => m,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return out,
        Err(err) => {
            out.diagnostics
                .push(diagnostic(path, 0, "error", format!("stat failed: {err}")));
            return out;
        }
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return out;
    }
    if metadata.len() > limit {
        out.diagnostics.push(diagnostic(
            path,
            0,
            "warning",
            format!("file exceeds --max-file-bytes ({limit})"),
        ));
        return out;
    }
    let data = match fs::read(&full_path) {
        Ok(data) => data,
        Err(err) => {
            out.diagnostics
                .push(diagnostic(path, 0, "error", format!("read failed: {err}")));
            return out;
        }
    };
    out.read_ok = true;
    if memmem::find(&data, b"@mark").is_none() || data.contains(&0) {
        return out;
    }
    let source = match std::str::from_utf8(&data) {
        Ok(s) => s,
        Err(_) => {
            out.diagnostics.push(diagnostic(
                path,
                0,
                "warning",
                "non-UTF-8 file containing @mark",
            ));
            return out;
        }
    };
    let lines: Vec<&str> = source.lines().collect();
    let syntax = syntax_for(path).expect("filtered by extension");
    let mut lexer = Lexer::new(syntax, path.ends_with(".rs"));
    for (idx, line) in lines.iter().enumerate() {
        for span in lexer.comments(line) {
            let raw = line[span.start..span.end].trim();
            let raw = raw.strip_prefix('*').unwrap_or(raw).trim_start();
            if !raw.starts_with("@mark") {
                continue;
            }
            if !raw[5..].is_empty() && !raw.as_bytes()[5].is_ascii_whitespace() {
                continue;
            }
            match parse_mark(raw) {
                Ok(fields) => {
                    let target = find_target(&lines, idx, span.comment_start, syntax);
                    if target.is_none() {
                        out.diagnostics.push(diagnostic(
                            path,
                            idx + 1,
                            "error",
                            "mark has no nearby code target (within 24 lines)",
                        ));
                    }
                    let symbol = target.and_then(|target_idx| symbol_from_line(lines[target_idx]));
                    out.marks.push(Mark {
                        label: fields.label,
                        path: path.to_owned(),
                        line: idx + 1,
                        target_line: target.map(|i| i + 1),
                        symbol,
                        role: fields.role,
                        contract: fields.contract,
                        summary: nearby_summary(&lines, idx, syntax),
                        snippet: if include_snippets {
                            target.map(|i| code_snippet(&lines, idx, i))
                        } else {
                            None
                        },
                    });
                }
                Err(error) => out
                    .diagnostics
                    .push(diagnostic(path, idx + 1, "error", error)),
            }
        }
    }
    out
}

struct Fields {
    label: String,
    role: Option<String>,
    contract: Option<String>,
}

fn valid_label(s: &str) -> bool {
    s.len() <= 128
        && s.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .next()
                    .is_some_and(|b| b.is_ascii_alphanumeric())
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
}

fn parse_mark(s: &str) -> std::result::Result<Fields, String> {
    let mut words = s[5..].split_whitespace();
    let label = words.next().ok_or("missing label after @mark")?;
    if !valid_label(label) {
        return Err(format!("invalid label: {label} (use dot-separated words)"));
    }
    let mut fields = Fields {
        label: label.to_owned(),
        role: None,
        contract: None,
    };
    let mut seen = HashSet::new();
    for item in words {
        let (key, value) = item
            .split_once('=')
            .ok_or_else(|| format!("invalid field: {item} (expected key=value)"))?;
        if value.is_empty() {
            return Err(format!("empty value for {key}"));
        }
        if !seen.insert(key) {
            return Err(format!("duplicate field: {key}"));
        }
        match key {
            "role" => fields.role = Some(value.to_owned()),
            "contract" => fields.contract = Some(value.to_owned()),
            _ => return Err(format!("unknown field: {key} (supported: role, contract)")),
        }
    }
    Ok(fields)
}

#[derive(Clone, Copy)]
enum Quote {
    Simple(u8),
    Triple(u8),
}

struct Span {
    comment_start: usize,
    start: usize,
    end: usize,
}

struct Lexer {
    syntax: Syntax,
    rust: bool,
    block_end: Option<&'static [u8]>,
    quote: Option<Quote>,
}

impl Lexer {
    fn new(syntax: Syntax, rust: bool) -> Self {
        Self {
            syntax,
            rust,
            block_end: None,
            quote: None,
        }
    }

    fn comments(&mut self, line: &str) -> Vec<Span> {
        let bytes = line.as_bytes();
        let mut i = 0;
        let mut result = Vec::new();
        while i < bytes.len() {
            if let Some(end) = self.block_end {
                if let Some(offset) = memmem::find(&bytes[i..], end) {
                    let stop = i + offset;
                    result.push(Span {
                        comment_start: i,
                        start: i,
                        end: stop,
                    });
                    i = stop + end.len();
                    self.block_end = None;
                    continue;
                }
                result.push(Span {
                    comment_start: i,
                    start: i,
                    end: bytes.len(),
                });
                break;
            }
            if let Some(quote) = self.quote {
                match quote {
                    Quote::Simple(delimiter) => {
                        if bytes[i] == b'\\' {
                            i += 2;
                        } else if bytes[i] == delimiter {
                            self.quote = None;
                            i += 1;
                        } else {
                            i += 1;
                        }
                    }
                    Quote::Triple(delimiter) => {
                        if bytes[i..].starts_with(&[delimiter, delimiter, delimiter]) {
                            self.quote = None;
                            i += 3;
                        } else if bytes[i] == b'\\' {
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                }
                continue;
            }
            let suffix = &bytes[i..];
            let block = match self.syntax {
                Syntax::Slash if suffix.starts_with(b"/*") => Some((2, b"*/".as_slice())),
                Syntax::Html if suffix.starts_with(b"<!--") => Some((4, b"-->".as_slice())),
                _ => None,
            };
            if let Some((width, end)) = block {
                let start = i + width;
                if let Some(offset) = memmem::find(&bytes[start..], end) {
                    let stop = start + offset;
                    result.push(Span {
                        comment_start: i,
                        start,
                        end: stop,
                    });
                    i = stop + end.len();
                } else {
                    result.push(Span {
                        comment_start: i,
                        start,
                        end: bytes.len(),
                    });
                    self.block_end = Some(end);
                    break;
                }
                continue;
            }
            let line_comment = match self.syntax {
                Syntax::Slash => suffix.starts_with(b"//").then_some(2),
                Syntax::Hash => suffix.starts_with(b"#").then_some(1),
                Syntax::Dash => suffix.starts_with(b"--").then_some(2),
                Syntax::Html => None,
            };
            if let Some(width) = line_comment {
                result.push(Span {
                    comment_start: i,
                    start: i + width,
                    end: bytes.len(),
                });
                break;
            }
            let delimiter = bytes[i];
            if delimiter == b'"' || delimiter == b'\'' || delimiter == b'\x60' {
                if self.rust && delimiter == b'\'' {
                    let char_literal = bytes.get(i + 2) == Some(&b'\'')
                        || (bytes.get(i + 1) == Some(&b'\\') && bytes.get(i + 3) == Some(&b'\''));
                    if !char_literal {
                        i += 1;
                        continue;
                    }
                }
                if matches!(self.syntax, Syntax::Hash)
                    && bytes[i..].starts_with(&[delimiter, delimiter, delimiter])
                {
                    self.quote = Some(Quote::Triple(delimiter));
                    i += 3;
                } else {
                    self.quote = Some(Quote::Simple(delimiter));
                    i += 1;
                }
                continue;
            }
            i += 1;
        }
        result
    }
}

fn is_comment_only(line: &str, syntax: Syntax) -> bool {
    let s = line.trim();
    if s.is_empty() {
        return true;
    }
    match syntax {
        Syntax::Slash => {
            s.starts_with("//") || s.starts_with("/*") || s.starts_with('*') || s == "*/"
        }
        Syntax::Hash => s.starts_with('#'),
        Syntax::Dash => s.starts_with("--"),
        Syntax::Html => s.starts_with("<!--") || s.starts_with("-->") || s.starts_with('*'),
    }
}

fn find_target(
    lines: &[&str],
    marker_idx: usize,
    comment_start: usize,
    syntax: Syntax,
) -> Option<usize> {
    if lines[marker_idx][..comment_start]
        .trim()
        .chars()
        .any(|c| c.is_alphanumeric())
    {
        return Some(marker_idx);
    }
    for idx in marker_idx + 1..lines.len().min(marker_idx + 25) {
        let line = lines[idx].trim();
        if is_comment_only(line, syntax) || line.starts_with("#[") || line.starts_with('@') {
            continue;
        }
        if line == "}" || line == "};" || line == "end" {
            return None;
        }
        return Some(idx);
    }
    None
}

fn plain_comment(line: &str, syntax: Syntax) -> Option<&str> {
    let s = line.trim();
    let s = match syntax {
        Syntax::Slash => s
            .strip_prefix("//")
            .or_else(|| s.strip_prefix("/*"))
            .or_else(|| s.strip_prefix('*')),
        Syntax::Hash => s.strip_prefix('#'),
        Syntax::Dash => s.strip_prefix("--"),
        Syntax::Html => s.strip_prefix("<!--").or_else(|| s.strip_prefix('*')),
    }?;
    let s = s
        .trim()
        .trim_end_matches("*/")
        .trim_end_matches("-->")
        .trim();
    if s.is_empty() || s.starts_with("@mark") {
        None
    } else {
        Some(s)
    }
}

fn nearby_summary(lines: &[&str], marker_idx: usize, syntax: Syntax) -> Option<String> {
    for distance in 1..=4 {
        if let Some(idx) = marker_idx.checked_sub(distance) {
            if let Some(comment) = plain_comment(lines[idx], syntax) {
                return Some(comment.chars().take(180).collect());
            }
            if !is_comment_only(lines[idx], syntax) {
                break;
            }
        }
    }
    for idx in marker_idx + 1..lines.len().min(marker_idx + 5) {
        if let Some(comment) = plain_comment(lines[idx], syntax) {
            return Some(comment.chars().take(180).collect());
        }
        if !is_comment_only(lines[idx], syntax) {
            break;
        }
    }
    None
}

fn symbol_from_line(line: &str) -> Option<String> {
    let mut s = line.trim();
    loop {
        let next = [
            "pub(crate) ",
            "pub(super) ",
            "pub ",
            "async ",
            "export ",
            "default ",
            "static ",
            "final ",
        ]
        .iter()
        .find_map(|prefix| s.strip_prefix(prefix));
        match next {
            Some(stripped) => s = stripped,
            None => break,
        }
    }
    for prefix in [
        "fn ",
        "func ",
        "def ",
        "function ",
        "class ",
        "struct ",
        "enum ",
        "interface ",
        "trait ",
        "impl ",
        "type ",
        "const ",
        "let ",
        "var ",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            let rest = if prefix == "func " && rest.starts_with('(') {
                rest.split_once(')')?.1.trim_start()
            } else {
                rest
            };
            let name = rest
                .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
                .next()
                .unwrap_or("");
            if !name.is_empty() {
                return Some(name.to_owned());
            }
        }
    }
    None
}

fn code_snippet(lines: &[&str], mark: usize, target: usize) -> String {
    let from = mark.saturating_sub(1);
    let to = (target + 7).min(lines.len());
    let mut snippet = String::new();
    for idx in from..to {
        let display = lines[idx].chars().take(200).collect::<String>();
        snippet.push_str(&format!("{:>5} | {}\n", idx + 1, display));
    }
    snippet
}

pub fn matches_query(mark: &Mark, query: &str) -> bool {
    let query = query.to_lowercase();
    [
        Some(mark.label.as_str()),
        Some(mark.path.as_str()),
        mark.summary.as_deref(),
        mark.symbol.as_deref(),
        mark.role.as_deref(),
        mark.contract.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|field| field.to_lowercase().contains(&query))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_text(text: &str, path: &str) -> FileResult {
        let dir = std::env::temp_dir().join(format!("markref-unit-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(path), text).unwrap();
        scan_file(&dir, path, 1024 * 1024, true)
    }

    #[test]
    fn repeated_function_labels_are_valid() {
        let result = scan_text(
            "// Refund entry\n// @mark payments.refund role=authority\nfn refund() {}\n\
             // @mark payments.refund role=test\nfn refund_test() {}\n",
            "demo.rs",
        );
        assert_eq!(result.marks.len(), 2);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.marks[0].symbol.as_deref(), Some("refund"));
        assert_eq!(result.marks[0].summary.as_deref(), Some("Refund entry"));
    }

    #[test]
    fn ignores_markers_in_strings_and_accepts_block_comments() {
        let result = scan_text(
            "let s = \"// @mark fake\";\n\
             let t = r#\"// @mark also.fake\"#;\n\
             /* @mark actual.ref contract=rpc:Commerce.Refund */\n\
             fn refund() {}\n",
            "strings.rs",
        );
        assert_eq!(result.marks.len(), 1, "{:?}", result.marks);
        assert_eq!(result.marks[0].label, "actual.ref");
    }

    #[test]
    fn supports_hash_comments_and_chinese_search() {
        let result = scan_text(
            "# 退款接口\n# @mark payments.refund role=caller\n\
             def refund():\n    return True\n",
            "sample.py",
        );
        assert_eq!(result.marks.len(), 1);
        assert!(matches_query(&result.marks[0], "退款"));
        assert_eq!(result.marks[0].symbol.as_deref(), Some("refund"));
    }

    #[test]
    fn rejects_bad_fields_and_orphans() {
        let bad = scan_text("// @mark a.b role=caller role=ui\nfn f() {}\n", "bad.rs");
        assert_eq!(bad.diagnostics.len(), 1);
        let orphan = scan_text("// @mark a.b\n", "orphan.rs");
        assert_eq!(orphan.marks.len(), 1);
        assert_eq!(orphan.diagnostics.len(), 1);
    }

    #[test]
    fn does_not_treat_python_triple_strings_as_comments() {
        let result = scan_text(
            "text = \"\"\"\n# @mark phantom\n\"\"\"\n# @mark real.label\n\
             def f():\n    pass\n",
            "triple.py",
        );
        assert_eq!(result.marks.len(), 1);
        assert_eq!(result.marks[0].label, "real.label");
    }

    #[test]
    fn does_not_bind_across_closing_braces() {
        let result = scan_text("fn foo() {\n  // @mark broken\n}\n", "unbound.rs");
        assert_eq!(result.diagnostics.len(), 1);
    }
}
