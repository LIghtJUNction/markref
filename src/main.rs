mod scan;

use std::path::PathBuf;

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
    /// Directory within the Git project to scan.
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
    /// Search marks by label, nearby documentation, file, symbol, role, or contract.
    Find {
        /// Case-insensitive search text, including Unicode.
        query: String,
    },
    /// Show every location sharing an exact feature label.
    Show { label: String },
    /// Report invalid marks and marks without a nearby code target.
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

fn run() -> Result<()> {
    let cli = Cli::parse();
    let include_snippets = matches!(&cli.command, Commands::Context { expand: true, .. });
    let report = scan::scan_project(
        &cli.root,
        cli.include_untracked,
        cli.max_file_bytes,
        include_snippets,
    )?;

    match cli.command {
        Commands::Find { query } => {
            let found: Vec<&Mark> = report
                .marks
                .iter()
                .filter(|mark| scan::matches_query(mark, &query))
                .collect();
            if found.is_empty() {
                bail!("no marks found for query: {query}");
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
        bail!("no marks found for label: {label}");
    }
    Ok(found)
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
            println!(
                "{}:{}  {}  role={} contract={} symbol={} {}",
                mark.path, mark.line, mark.label, role, contract, symbol, summary
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
        output.push_str(&format!(
            "\n{}:{} -> {}  role={}  contract={}  symbol={}\n",
            mark.path,
            mark.line,
            target,
            mark.role.as_deref().unwrap_or("-"),
            mark.contract.as_deref().unwrap_or("-"),
            mark.symbol.as_deref().unwrap_or("-")
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
            "locations": included,
            "truncated": false
        });
        if serde_json::to_vec(&candidate)?.len() > max_bytes {
            included.pop();
            truncated = true;
            break;
        }
    }
    let value = serde_json::json!({
        "label": label,
        "locations": included,
        "truncated": truncated
    });
    let output = serde_json::to_string(&value)?;
    if output.len() > max_bytes {
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
