mod scan;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use serde::Serialize;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use scan::{Mark, Report};

#[derive(Parser)]
#[command(
    name = "markref",
    version,
    about = "Locate shared feature marks in source comments"
)]
struct Cli {
    /// Directory within a Git worktree; scans the whole worktree.
    #[arg(long, global = true, default_value = ".", value_name = "DIR")]
    root: PathBuf,

    /// Also scan untracked, non-ignored files.
    #[arg(long, global = true)]
    include_untracked: bool,

    /// Emit JSON instead of plain text.
    #[arg(long, global = true)]
    json: bool,

    /// Maximum size of each scanned source file.
    #[arg(long, global = true, default_value_t = 16 * 1024 * 1024)]
    max_file_bytes: u64,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List feature labels with location counts and roles.
    List,
    /// Search marks by label, nearby documentation, file, symbol, role, or contract.
    Find {
        /// Case-insensitive search text, including Unicode.
        query: String,
    },
    /// Show every location sharing an exact feature label.
    Show { label: String },
    /// Report invalid markers and feature marks without a nearby code target.
    Check,
    /// Show a compact map of all locations for a feature.
    Context {
        label: String,

        /// Maximum number of output bytes (valid UTF-8).
        #[arg(long, default_value_t = 8000)]
        max_bytes: usize,

        /// Include bounded code excerpts around each target.
        #[arg(long)]
        expand: bool,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

// @mark markref.cli role=authority
fn run() -> Result<()> {
    let cli = Cli::parse();
    let include_snippets = matches!(&cli.command, Commands::Context { expand: true, .. });
    let report = scan::scan_project(
        &cli.root,
        cli.include_untracked,
        cli.max_file_bytes,
        include_snippets,
    )?;

    if !matches!(&cli.command, Commands::Check) && !report.diagnostics.is_empty() {
        let errors = report
            .diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .count();
        let warnings = report.diagnostics.len() - errors;
        eprintln!("warning: scan found {errors} errors and {warnings} warnings; results may be incomplete (run markref check)");
    }

    match cli.command {
        Commands::List => print_labels(&report.marks, cli.json)?,
        Commands::Find { query } => {
            let found: Vec<&Mark> = report
                .marks
                .iter()
                .filter(|mark| scan::matches_query(mark, &query))
                .collect();
            if found.is_empty() {
                bail!("no marks found for query: {query}; use 'markref list' to discover labels or rg to search unmarked code");
            }
            print_marks(&found, cli.json)?;
        }
        Commands::Show { label } => {
            let found = by_label(&report, &label)?;
            print_marks(&found, cli.json)?;
        }
        Commands::Check => {
            let errors = report
                .diagnostics
                .iter()
                .filter(|d| d.severity == "error")
                .count();
            let warnings = report.diagnostics.len() - errors;
            if cli.json {
                let value = serde_json::json!({
                    "files_scanned": report.files_scanned,
                    "marks": report.marks.len(),
                    "errors": errors,
                    "warnings": warnings,
                    "diagnostics": report.diagnostics
                });
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                for item in &report.diagnostics {
                    eprintln!(
                        "{}:{}: {}: {}",
                        item.path, item.line, item.severity, item.message
                    );
                }
                println!(
                    "{} marks in {} files; {} errors, {} warnings",
                    report.marks.len(),
                    report.files_scanned,
                    errors,
                    warnings
                );
            }
            if errors != 0 {
                bail!("mark validation failed");
            }
        }
        Commands::Context {
            label,
            max_bytes,
            expand,
        } => {
            if max_bytes < 64 {
                bail!("--max-bytes must be at least 64");
            }
            let found = by_label(&report, &label)?;
            if cli.json {
                print_context_json(&label, &found, max_bytes)?;
            } else {
                print_context_text(&label, &found, max_bytes, expand);
            }
        }
    }
    Ok(())
}

fn by_label<'a>(report: &'a Report, label: &str) -> Result<Vec<&'a Mark>> {
    let found: Vec<&Mark> = report
        .marks
        .iter()
        .filter(|mark| mark.label == label)
        .collect();
    if found.is_empty() {
        bail!("no marks found for label: {label}; use 'markref list' to discover exact labels");
    }
    Ok(found)
}

#[derive(Serialize)]
struct LabelSummary<'a> {
    label: &'a str,
    locations: usize,
    roles: BTreeSet<&'a str>,
}

fn print_labels(marks: &[Mark], json: bool) -> Result<()> {
    let mut labels = BTreeMap::new();
    for mark in marks {
        let summary = labels
            .entry(mark.label.as_str())
            .or_insert_with(|| LabelSummary {
                label: &mark.label,
                locations: 0,
                roles: BTreeSet::new(),
            });
        summary.locations += 1;
        if let Some(role) = &mark.role {
            summary.roles.insert(role.as_str());
        }
    }
    let labels = labels.into_values().collect::<Vec<_>>();
    if json {
        println!("{}", serde_json::to_string_pretty(&labels)?);
    } else if labels.is_empty() {
        println!("No feature marks found in scanned source.");
    } else {
        for summary in labels {
            let roles = summary.roles.iter().copied().collect::<Vec<_>>().join(",");
            let roles = if roles.is_empty() { "-" } else { &roles };
            println!(
                "{}  locations={} roles={}",
                summary.label, summary.locations, roles
            );
        }
    }
    Ok(())
}

fn print_marks(marks: &[&Mark], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(marks)?);
    } else {
        for mark in marks {
            let symbol = mark.symbol.as_deref().unwrap_or("-");
            let role = mark.role.as_deref().unwrap_or("-");
            let contract = mark.contract.as_deref().unwrap_or("-");
            let summary = mark.summary.as_deref().unwrap_or("");
            let boundary = mark
                .fold_boundary
                .map(|boundary| format!(" fold_boundary={boundary}"))
                .unwrap_or_default();
            println!(
                "{}:{}  {}  role={} contract={} symbol={}{} {}",
                mark.path, mark.line, mark.label, role, contract, symbol, boundary, summary
            );
        }
    }
    Ok(())
}

fn print_context_text(label: &str, marks: &[&Mark], max_bytes: usize, expand: bool) {
    let mut output = format!("{label} ({} locations)\n", marks.len());
    for mark in marks {
        let target = mark
            .target_line
            .map_or_else(|| "unbound".to_owned(), |line| line.to_string());
        let boundary = mark
            .fold_boundary
            .map(|boundary| format!("  fold_boundary={boundary}"))
            .unwrap_or_default();
        output.push_str(&format!(
            "\n{}:{} -> {}  role={}  contract={}  symbol={}{}\n",
            mark.path,
            mark.line,
            target,
            mark.role.as_deref().unwrap_or("-"),
            mark.contract.as_deref().unwrap_or("-"),
            mark.symbol.as_deref().unwrap_or("-"),
            boundary
        ));
        if let Some(summary) = &mark.summary {
            output.push_str(summary);
            output.push('\n');
        }
        if expand {
            if let Some(snippet) = &mark.snippet {
                output.push_str(snippet);
            }
        }
        if output.len() > max_bytes {
            break;
        }
    }
    clamp_utf8(&mut output, max_bytes);
    print!("{output}");
}

fn print_context_json(label: &str, marks: &[&Mark], max_bytes: usize) -> Result<()> {
    let mut included: Vec<&Mark> = Vec::new();
    let mut truncated = false;
    for mark in marks {
        included.push(mark);
        let candidate = serde_json::json!({
            "label": label,
            "total_locations": marks.len(),
            "locations": included,
            "truncated": false
        });
        // println! adds one byte outside the serialized JSON value.
        if serde_json::to_vec(&candidate)?.len().saturating_add(1) > max_bytes {
            included.pop();
            truncated = true;
            break;
        }
    }
    let value = serde_json::json!({
        "label": label,
        "total_locations": marks.len(),
        "locations": included,
        "truncated": truncated
    });
    let output = serde_json::to_string(&value)?;
    if output.len().saturating_add(1) > max_bytes {
        bail!("--max-bytes is too small for the JSON header; increase the limit");
    }
    println!("{output}");
    Ok(())
}

fn clamp_utf8(output: &mut String, max_bytes: usize) {
    if output.len() <= max_bytes {
        return;
    }
    let suffix = "\n[truncated]\n";
    let with_suffix = max_bytes >= suffix.len();
    let mut keep = if with_suffix {
        max_bytes - suffix.len()
    } else {
        max_bytes
    };
    while !output.is_char_boundary(keep) {
        keep -= 1;
    }
    output.truncate(keep);
    if with_suffix {
        output.push_str(suffix);
    }
}
