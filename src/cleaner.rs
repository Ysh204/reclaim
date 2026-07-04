use crate::model::Finding;
use anyhow::Result;
use colored::Colorize;
use std::fs;

/// Delete each finding's directory. Continues past individual failures so
/// one locked file doesn't abort the whole cleanup; reports a summary.
pub fn delete_findings(findings: &[Finding]) -> Result<(u64, usize)> {
    let mut freed: u64 = 0;
    let mut failures = 0usize;

    for finding in findings {
        match fs::remove_dir_all(&finding.path) {
            Ok(()) => {
                freed += finding.size_bytes;
                println!("  {} {}", "removed".green(), finding.path.display());
            }
            Err(e) => {
                failures += 1;
                println!(
                    "  {} {} ({e})",
                    "failed to remove".red(),
                    finding.path.display()
                );
            }
        }
    }

    Ok((freed, failures))
}
