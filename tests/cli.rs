//! Integration tests that run the actual compiled `reclaim` binary against
//! disposable temp directories. These test the whole pipeline end to end —
//! CLI parsing, scanning, sizing, and (for the --yes case) real deletion.

use std::fs;
use std::path::Path;
use std::process::Command;

fn reclaim_bin() -> &'static str {
    env!("CARGO_BIN_EXE_reclaim")
}

fn write_file(path: &Path, bytes: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, vec![0u8; bytes]).unwrap();
}

#[test]
fn dry_run_reports_but_does_not_delete() {
    let dir = tempfile::tempdir().unwrap();
    let nm = dir.path().join("proj/node_modules/pkg/blob.bin");
    write_file(&nm, 1_000_000);

    let output = Command::new(reclaim_bin())
        .arg(dir.path())
        .arg("--dry-run")
        .arg("--min-size-mb")
        .arg("0")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("node_modules") || stdout.contains("Node.js"));
    assert!(stdout.contains("Dry run"));

    // The critical assertion: the file must still exist.
    assert!(nm.exists());
}

#[test]
fn yes_flag_actually_deletes_matched_directories() {
    let dir = tempfile::tempdir().unwrap();
    let nm_file = dir.path().join("proj/node_modules/pkg/blob.bin");
    let src_file = dir.path().join("proj/src/index.js");
    write_file(&nm_file, 1_000_000);
    write_file(&src_file, 10);

    let output = Command::new(reclaim_bin())
        .arg(dir.path())
        .arg("--yes")
        .arg("--min-size-mb")
        .arg("0")
        .output()
        .unwrap();

    assert!(output.status.success());

    // node_modules should be gone...
    assert!(!dir.path().join("proj/node_modules").exists());
    // ...but untouched source files must survive.
    assert!(src_file.exists());
}

#[test]
fn decoy_build_directory_is_never_touched() {
    let dir = tempfile::tempdir().unwrap();
    let decoy = dir.path().join("proj/build/output.txt");
    write_file(&decoy, 5_000_000);

    let output = Command::new(reclaim_bin())
        .arg(dir.path())
        .arg("--yes")
        .arg("--min-size-mb")
        .arg("0")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Nothing found"));
    assert!(decoy.exists());
}

#[test]
fn min_size_filter_hides_small_findings() {
    let dir = tempfile::tempdir().unwrap();
    write_file(&dir.path().join("proj/node_modules/pkg/tiny.bin"), 1_000);

    let output = Command::new(reclaim_bin())
        .arg(dir.path())
        .arg("--dry-run")
        .arg("--min-size-mb")
        .arg("1") // 1MB threshold, finding is ~1KB
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nothing to show") || stdout.contains("Nothing found"));
}

#[test]
fn json_output_is_valid_and_matches_findings() {
    let dir = tempfile::tempdir().unwrap();
    write_file(
        &dir.path().join("proj/node_modules/pkg/blob.bin"),
        2_000_000,
    );

    let output = Command::new(reclaim_bin())
        .arg(dir.path())
        .arg("--json")
        .arg("--min-size-mb")
        .arg("0")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // First line is the "Scanning ..." banner; the rest is JSON.
    let json_start = stdout.find('[').expect("expected a JSON array in output");
    let json_str = &stdout[json_start..];
    let parsed: serde_json::Value = serde_json::from_str(json_str).expect("valid JSON");

    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["label"], "Node.js");
    assert_eq!(arr[0]["size_bytes"], 2_000_000);
}

#[test]
fn nonexistent_path_exits_with_error() {
    let output = Command::new(reclaim_bin())
        .arg("/this/path/definitely/does/not/exist")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error"));
}

#[test]
fn exclude_flag_skips_matching_directories() {
    let dir = tempfile::tempdir().unwrap();
    let nm1 = dir.path().join("proj1/node_modules/pkg/blob.bin");
    let nm2 = dir.path().join("proj2/node_modules/pkg/blob.bin");
    write_file(&nm1, 1_000_000);
    write_file(&nm2, 1_000_000);

    let output = Command::new(reclaim_bin())
        .arg(dir.path())
        .arg("--yes")
        .arg("--min-size-mb")
        .arg("0")
        .arg("--exclude")
        .arg("proj1")
        .output()
        .unwrap();

    assert!(output.status.success());

    // proj1's node_modules should still exist since it was excluded...
    assert!(nm1.exists());
    // ...but proj2's node_modules should be gone.
    assert!(!nm2.exists());
}

#[test]
fn reclaimignore_file_skips_matching_directories() {
    let dir = tempfile::tempdir().unwrap();
    let nm1 = dir.path().join("proj1/node_modules/pkg/blob.bin");
    let nm2 = dir.path().join("proj2/node_modules/pkg/blob.bin");
    write_file(&nm1, 1_000_000);
    write_file(&nm2, 1_000_000);

    // Create a .reclaimignore file at the root
    fs::write(dir.path().join(".reclaimignore"), "proj1\n# some comment\n  \n").unwrap();

    let output = Command::new(reclaim_bin())
        .arg(dir.path())
        .arg("--yes")
        .arg("--min-size-mb")
        .arg("0")
        .output()
        .unwrap();

    assert!(output.status.success());

    // proj1's node_modules should still exist since it was excluded by .reclaimignore...
    assert!(nm1.exists());
    // ...but proj2's node_modules should be gone.
    assert!(!nm2.exists());
}

#[test]
fn cli_accepts_sort_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let nm = dir.path().join("proj/node_modules/pkg/blob.bin");
    write_file(&nm, 1_000_000);

    let output_size = Command::new(reclaim_bin())
        .arg(dir.path())
        .arg("--dry-run")
        .arg("--min-size-mb")
        .arg("0")
        .arg("--sort")
        .arg("size")
        .output()
        .unwrap();
    assert!(output_size.status.success());

    let output_age = Command::new(reclaim_bin())
        .arg(dir.path())
        .arg("--dry-run")
        .arg("--min-size-mb")
        .arg("0")
        .arg("--sort")
        .arg("age")
        .output()
        .unwrap();
    assert!(output_age.status.success());
}

#[test]
fn skip_recent_filters_out_recently_modified_directories() {
    let dir = tempfile::tempdir().unwrap();
    let nm1 = dir.path().join("proj1/node_modules/pkg/blob.bin");
    let nm2 = dir.path().join("proj2/node_modules/pkg/blob.bin");

    // Write nm1 and set its modified time to 5 days ago
    write_file(&nm1, 1_000_000);
    let f1 = std::fs::OpenOptions::new().write(true).open(&nm1).unwrap();
    let time_5_days_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(5 * 86_400);
    f1.set_modified(time_5_days_ago).unwrap();

    // Write nm2 and leave it at current time (0 days ago)
    write_file(&nm2, 1_000_000);

    // If we skip recent (< 3 days), proj2/node_modules should be skipped because it is 0 days old.
    // proj1/node_modules is 5 days old, so it should be kept and deleted.
    let output = Command::new(reclaim_bin())
        .arg(dir.path())
        .arg("--yes")
        .arg("--min-size-mb")
        .arg("0")
        .arg("--skip-recent")
        .arg("3")
        .output()
        .unwrap();

    assert!(output.status.success());

    // proj1's node_modules should be deleted...
    assert!(!nm1.exists());
    // ...but proj2's node_modules should still exist since it was skipped.
    assert!(nm2.exists());
}
