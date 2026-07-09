use crate::detectors::{DETECTORS, SKIP_DESCENDING};
use crate::model::Finding;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::time::Duration;
use walkdir::WalkDir;

/// True if `dir` has a sibling file matching `marker` in `parent`.
/// `marker` is either an exact filename ("Cargo.toml") or a bare extension
/// like ".csproj", in which case any file with that extension counts.
fn has_marker_sibling(parent: &Path, marker: &str) -> bool {
    let Ok(entries) = fs::read_dir(parent) else {
        return false;
    };
    let is_ext_pattern = marker.starts_with('.') && !marker[1..].contains('.');

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if is_ext_pattern {
            if name.ends_with(marker) && name != marker {
                // e.g. "MyApp.csproj" ends with ".csproj"
                return true;
            }
        } else if name == marker {
            return true;
        }
    }
    false
}

/// Walk `root`, looking for directories matching any known detector.
/// Returns unsized findings (label + path); size/mtime are filled in later
/// by `size_findings` so the initial walk stays fast.
pub fn find_candidates(root: &Path, excludes: &[String]) -> Vec<(String, String, std::path::PathBuf)> {
    let mut candidates = Vec::new();
    let mut it = WalkDir::new(root).into_iter();

    loop {
        let entry = match it.next() {
            Some(Ok(e)) => e,
            Some(Err(_)) => continue, // permission errors etc — skip silently
            None => break,
        };

        if !entry.file_type().is_dir() {
            continue;
        }

        let path_str = entry.path().to_string_lossy().replace('\\', "/");
        if excludes.iter().any(|pattern| {
            let normalized_pattern = pattern.replace('\\', "/");
            path_str.contains(&normalized_pattern)
        }) {
            it.skip_current_dir();
            continue;
        }

        let dir_name = entry.file_name().to_string_lossy();

        if SKIP_DESCENDING.contains(&dir_name.as_ref()) {
            it.skip_current_dir();
            continue;
        }

        let parent = entry.path().parent();

        for det in DETECTORS {
            if det.dir_name != dir_name.as_ref() {
                continue;
            }
            let marker_ok = match det.marker_sibling {
                None => true,
                Some(marker) => parent
                    .map(|p| has_marker_sibling(p, marker))
                    .unwrap_or(false),
            };
            if marker_ok {
                candidates.push((
                    det.label.to_string(),
                    det.regenerate_hint.to_string(),
                    entry.path().to_path_buf(),
                ));
                break;
            }
        }

        // Whether matched or not, once we've decided a directory's fate we
        // don't need to descend into a *matched* artifact directory — its
        // contents are irrelevant, we only care about its total size.
        if candidates
            .last()
            .map(|(_, _, p)| p == entry.path())
            .unwrap_or(false)
        {
            it.skip_current_dir();
        }
    }

    candidates
}

/// Compute total size (bytes) and most recent mtime for each candidate,
/// in parallel, with a progress spinner since this is the slow part.
pub fn size_findings(candidates: Vec<(String, String, std::path::PathBuf)>) -> Vec<Finding> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message(format!("Measuring {} directories...", candidates.len()));

    let findings: Vec<Finding> = candidates
        .into_par_iter()
        .map(|(label, regenerate_hint, path)| {
            let mut size_bytes: u64 = 0;
            let mut newest_secs: Option<u64> = None;

            for entry in WalkDir::new(&path).into_iter().flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        size_bytes += meta.len();
                        if let Ok(modified) = meta.modified() {
                            if let Ok(secs) = modified
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                            {
                                newest_secs = Some(newest_secs.map_or(secs, |n: u64| n.max(secs)));
                            }
                        }
                    }
                }
            }

            Finding {
                path,
                label,
                size_bytes,
                last_modified_secs: newest_secs,
                regenerate_hint,
            }
        })
        .collect();

    pb.finish_and_clear();
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        File::create(path).unwrap();
    }

    #[test]
    fn marker_sibling_exact_filename_match() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("Cargo.toml"));
        assert!(has_marker_sibling(dir.path(), "Cargo.toml"));
    }

    #[test]
    fn marker_sibling_missing_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_marker_sibling(dir.path(), "Cargo.toml"));
    }

    #[test]
    fn marker_sibling_extension_pattern_matches_any_basename() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("MyApp.csproj"));
        assert!(has_marker_sibling(dir.path(), ".csproj"));
    }

    #[test]
    fn marker_sibling_extension_pattern_does_not_match_bare_dotfile() {
        // A file literally named ".csproj" (no basename) should not count —
        // it's not a project file, it's an odd dotfile.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join(".csproj"));
        assert!(!has_marker_sibling(dir.path(), ".csproj"));
    }

    #[test]
    fn marker_sibling_unreadable_parent_returns_false() {
        // A path that doesn't exist at all should fail closed, not panic.
        let missing = Path::new("/this/path/should/not/exist/hopefully");
        assert!(!has_marker_sibling(missing, "Cargo.toml"));
    }

    #[test]
    fn finds_node_modules_without_marker() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("proj/node_modules/pkg/index.js"));

        let candidates = find_candidates(dir.path(), &[]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, "Node.js");
        assert_eq!(candidates[0].2, dir.path().join("proj/node_modules"));
    }

    #[test]
    fn finds_rust_target_only_with_cargo_toml_marker() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("proj/Cargo.toml"));
        touch(&dir.path().join("proj/target/debug/binary"));

        let candidates = find_candidates(dir.path(), &[]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, "Rust");
    }

    #[test]
    fn ignores_generic_build_dir_without_marker() {
        // A "build" directory with no build.gradle / build.gradle.kts
        // sibling should never be flagged — this is the core false-positive
        // guard and the most important thing to keep correct.
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("proj/build/output.txt"));

        let candidates = find_candidates(dir.path(), &[]);
        assert!(candidates.is_empty());
    }

    #[test]
    fn does_not_descend_into_matched_directory() {
        // If we matched node_modules as one unit, we shouldn't also report
        // a nested node_modules-looking thing inside it as a second finding.
        let dir = tempfile::tempdir().unwrap();
        touch(
            &dir.path()
                .join("proj/node_modules/nested/node_modules/x.js"),
        );

        let candidates = find_candidates(dir.path(), &[]);
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn skips_descending_into_git_directory() {
        let dir = tempfile::tempdir().unwrap();
        // Put something inside .git that would match node_modules if we
        // (wrongly) walked into it.
        touch(&dir.path().join(".git/node_modules/x.js"));

        let candidates = find_candidates(dir.path(), &[]);
        assert!(candidates.is_empty());
    }

    #[test]
    fn sizes_findings_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let nm = dir.path().join("proj/node_modules");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join("a.bin"), vec![0u8; 1000]).unwrap();
        fs::write(nm.join("b.bin"), vec![0u8; 2000]).unwrap();

        let candidates = find_candidates(dir.path(), &[]);
        let findings = size_findings(candidates);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].size_bytes, 3000);
        assert!(findings[0].last_modified_secs.is_some());
    }

    #[test]
    fn finds_flutter_dart_tool() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("flutter_proj/pubspec.yaml"));
        touch(&dir.path().join("flutter_proj/.dart_tool/package_config.json"));

        let candidates = find_candidates(dir.path(), &[]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, "Flutter / Dart");
    }

    #[test]
    fn finds_swift_pm_build() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("swift_proj/Package.swift"));
        touch(&dir.path().join("swift_proj/.build/debug/foo"));

        let candidates = find_candidates(dir.path(), &[]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, "Swift Package Manager");
    }

    #[test]
    fn excludes_paths_properly() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("proj1/node_modules/x.js"));
        touch(&dir.path().join("proj2/node_modules/x.js"));

        // Exclude proj1 entirely
        let candidates = find_candidates(dir.path(), &["proj1".to_string()]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].2, dir.path().join("proj2/node_modules"));

        // Exclude node_modules
        let candidates = find_candidates(dir.path(), &["node_modules".to_string()]);
        assert!(candidates.is_empty());
    }
}
