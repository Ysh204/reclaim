use crate::model::Finding;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use humansize::{format_size, DECIMAL};

/// Print a sorted, human-readable table of findings to stdout.
pub fn print_table(findings: &[Finding]) {
    let total: u64 = findings.iter().map(|f| f.size_bytes).sum();

    println!();
    println!(
        "{:<10} {:<22} {:<10} {}",
        "SIZE".bold(),
        "TYPE".bold(),
        "AGE".bold(),
        "PATH".bold()
    );
    println!("{}", "-".repeat(80));

    for f in findings {
        let age = match f.age_days() {
            Some(0) => "today".to_string(),
            Some(d) => format!("{d}d ago"),
            None => "-".to_string(),
        };
        println!(
            "{:<10} {:<22} {:<10} {}",
            format_size(f.size_bytes, DECIMAL).cyan(),
            f.label,
            age,
            f.path.display()
        );
        println!(
            "{:<10} {:<22} {:<10} {} {}",
            "",
            "",
            "",
            "↳ restore with:".dimmed(),
            f.regenerate_hint.dimmed()
        );
    }

    println!("{}", "-".repeat(80));
    println!("{}", "Ecosystem Breakdown:".bold());
    use std::collections::BTreeMap;
    let mut breakdown: BTreeMap<&str, (u64, usize)> = BTreeMap::new();
    for f in findings {
        let entry = breakdown.entry(&f.label).or_insert((0, 0));
        entry.0 += f.size_bytes;
        entry.1 += 1;
    }
    let mut breakdown_vec: Vec<(&&str, &(u64, usize))> = breakdown.iter().collect();
    breakdown_vec.sort_by(|a, b| b.1.0.cmp(&a.1.0)); // sort by size descending

    for (label, (size, count)) in breakdown_vec {
        let dir_word = if *count == 1 { "directory" } else { "directories" };
        println!(
            "  {:<20} {:<10} ({} {})",
            label.cyan(),
            format_size(*size, DECIMAL).yellow(),
            count,
            dir_word
        );
    }

    println!("{}", "-".repeat(80));
    println!(
        "{} reclaimable across {} directories\n",
        format_size(total, DECIMAL).yellow().bold(),
        findings.len()
    );
}

/// Show an interactive checklist so the user can pick exactly what to
/// delete. Returns the indices (into `findings`) that were checked.
/// Everything is pre-selected by default since these are all
/// known-regenerable artifacts.
pub fn select_findings(findings: &[Finding]) -> anyhow::Result<Vec<usize>> {
    let items: Vec<String> = findings
        .iter()
        .map(|f| {
            format!(
                "{:<10} {:<22} {}",
                format_size(f.size_bytes, DECIMAL),
                f.label,
                f.path.display()
            )
        })
        .collect();

    let defaults = vec![true; items.len()];

    let selection = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Space to toggle, Enter to confirm deletion")
        .items(&items)
        .defaults(&defaults)
        .interact()?;

    Ok(selection)
}
