use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git").arg("-C").arg(root).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn project() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::create_dir_all(dir.path().join("vendor")).unwrap();
    fs::write(dir.path().join(".gitignore"), "vendor/\n").unwrap();
    fs::write(
        dir.path().join("src/main.rs"),
        "// 退款入口\n// @mark payments.refund role=authority\nfn refund() {}\n\
         // @mark payments.refund role=caller contract=rpc:Commerce.Refund\nfn call() {}\n",
    ).unwrap();
    fs::write(
        dir.path().join("src/aux.py"),
        "# UI for refunds\n# @mark payments.refund role=ui\n\
         def refund_page():\n    pass\n",
    ).unwrap();
    fs::write(
        dir.path().join("vendor/ignored.rs"),
        "// @mark payments.refund role=vendor\nfn ignored() {}\n",
    ).unwrap();
    fs::write(
        dir.path().join("extra.rs"),
        "// @mark payments.refund role=untracked\nfn extra() {}\n",
    ).unwrap();
    git(dir.path(), &["add", ".gitignore", "src/main.rs", "src/aux.py"]);
    dir
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_markref"))
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn tracked_files_are_default_and_repeated_labels_are_allowed() {
    let root = project();
    let result = run(root.path(), &["show", "payments.refund"]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let text = stdout(&result);
    assert_eq!(text.matches("payments.refund").count(), 3);
    assert!(text.contains("src/main.rs"));
    assert!(text.contains("src/aux.py"));
    assert!(!text.contains("vendor"));
    assert!(!text.contains("untracked"));
    let checked = run(root.path(), &["check"]);
    assert!(checked.status.success(), "{}", String::from_utf8_lossy(&checked.stderr));
}

#[test]
fn include_untracked_still_respects_gitignore() {
    let root = project();
    let output = run(root.path(), &["--include-untracked", "show", "payments.refund"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert_eq!(text.matches("payments.refund").count(), 4);
    assert!(text.contains("role=untracked"));
    assert!(!text.contains("role=vendor"));
}

#[test]
fn chinese_search_finds_nearby_documentation() {
    let root = project();
    let output = run(root.path(), &["find", "退款"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("src/main.rs"));
    assert!(!text.contains("src/aux.py"));
}

#[test]
fn json_is_machine_readable() {
    let root = project();
    let output = run(root.path(), &["show", "payments.refund", "--json"]);
    assert!(output.status.success());
    let marks: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(marks.as_array().unwrap().len(), 3);
    assert_eq!(marks[0]["label"], "payments.refund");
}

#[test]
fn context_is_bounded_and_expands_on_request() {
    let root = project();
    let compact = run(root.path(), &["context", "payments.refund", "--max-bytes", "150"]);
    assert!(compact.status.success());
    assert!(compact.stdout.len() <= 150);
    assert!(std::str::from_utf8(&compact.stdout).is_ok());
    let expanded = run(root.path(), &["context", "payments.refund", "--expand"]);
    assert!(expanded.status.success());
    assert!(stdout(&expanded).contains("fn refund()"));
    let json = run(root.path(), &["context", "payments.refund", "--json", "--max-bytes", "300"]);
    assert!(json.status.success());
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert!(value["truncated"].as_bool().unwrap());
}

#[test]
fn bad_marks_make_check_fail_with_path_and_line() {
    let root = project();
    fs::write(
        root.path().join("src/bad.rs"),
        "// @mark\nfn bad() {}\n// @mark missing.target\n",
    ).unwrap();
    git(root.path(), &["add", "src/bad.rs"]);
    let result = run(root.path(), &["check"]);
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("src/bad.rs:1: error: missing label"), "{stderr}");
    assert!(stderr.contains("src/bad.rs:3: error: mark has no nearby code"), "{stderr}");
}

#[test]
fn no_git_worktree_is_reported() {
    let root = tempfile::tempdir().unwrap();
    let output = run(root.path(), &["check"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a Git worktree"));
}
