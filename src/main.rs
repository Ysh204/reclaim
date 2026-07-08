mod cleaner;
mod detectors;
mod model;
mod scanner;
mod ui;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use humansize::{format_size, DECIMAL};
use std::path::PathBuf;

/// reclaim — find and clean up disk space wasted by build artifacts,
/// dependency caches, and other regenerable dev-tool junk.
#[derive(Parser, Debug)]
#[command(name = "reclaim", version, about)]
struct Args {
    /// Directory to scan (defaults to the current directory).
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Hide findings smaller than this many megabytes.
    #[arg(long, default_value_t = 1)]
    min_size_mb: u64,

    /// Skip the interactive picker and delete every finding above
    /// --min-size-mb. Use with care.
    #[arg(long)]
    yes: bool,

    /// Only scan and report; never delete anything, even with --yes.
    #[arg(long)]
    dry_run: bool,

    /// Print findings as JSON instead of a human-readable table.
    #[arg(long)]
    json: bool,

    /// Exclude paths or directories containing any of these substrings.
    #[arg(long, short, value_delimiter = ',')]
    exclude: Vec<String>,

    /// Sort findings by: size (largest first), age (oldest first).
    #[arg(long, value_enum, default_value_t = SortBy::Size)]
    sort: SortBy,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum SortBy {
    Size,
    Age,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let root = args.path.canonicalize().unwrap_or(args.path.clone());
    if !root.is_dir() {
        eprintln!(
            "{} {} is not a directory",
            "error:".red().bold(),
            root.display()
        );
        std::process::exit(1);
    }

    let mut excludes = args.exclude.clone();
    if let Ok(ignore_content) = std::fs::read_to_string(root.join(".reclaimignore")) {
        for line in ignore_content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                excludes.push(trimmed.to_string());
            }
        }
    }

    println!("{} {}", "Scanning".bold(), root.display());
    let candidates = scanner::find_candidates(&root, &excludes);

    if candidates.is_empty() {
        println!("Nothing found — this tree looks clean already.");
        return Ok(());
    }

    let mut findings = scanner::size_findings(candidates);
    let min_bytes = args.min_size_mb * 1024 * 1024;
    findings.retain(|f| f.size_bytes >= min_bytes);

    match args.sort {
        SortBy::Size => findings.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes)),
        SortBy::Age => findings.sort_by(|a, b| {
            let age_a = a.age_days();
            let age_b = b.age_days();
            match (age_a, age_b) {
                (Some(da), Some(db)) => db.cmp(&da), // oldest first (larger days)
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        }),
    }

    if findings.is_empty() {
        println!(
            "Found only artifacts smaller than {} — nothing to show. Try --min-size-mb 0.",
            format_size(min_bytes, DECIMAL)
        );
        return Ok(());
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&findings)?);
        return Ok(());
    }

    ui::print_table(&findings);

    if args.dry_run {
        println!("{}", "Dry run — nothing was deleted.".yellow());
        return Ok(());
    }

    let to_delete: Vec<model::Finding> = if args.yes {
        findings
    } else {
        let selected_idx = ui::select_findings(&findings)?;
        if selected_idx.is_empty() {
            println!("Nothing selected — exiting without changes.");
            return Ok(());
        }
        selected_idx
            .into_iter()
            .map(|i| findings[i].clone())
            .collect()
    };

    println!("\n{}", "Deleting selected directories...".bold());
    let (freed, failures) = cleaner::delete_findings(&to_delete)?;

    println!();
    println!(
        "{} {}",
        "Freed:".green().bold(),
        format_size(freed, DECIMAL).green().bold()
    );
    if failures > 0 {
        println!(
            "{} {} item(s) could not be removed",
            "Warning:".yellow(),
            failures
        );
    }

    Ok(())
}
