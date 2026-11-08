// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cross-platform access to the orphan `maps` branch.
//!
//! The binary maps are generated artefacts and are not tracked on `master`.
//! They live on a single parentless branch named `maps`, so normal history
//! never accumulates superseded versions. This module is the only place that
//! knows how to move them:
//!
//! * [`fetch`] fetches the branch and writes its `maps/*.bin.gz` into the
//!   working tree (used automatically by the build script when they are
//!   missing);
//! * [`publish`] rebuilds the branch as one commit from the current
//!   `maps/*.bin.gz` and force-pushes it.
//!
//! Everything is `std::process::Command` against `git`, with paths and
//! environment built in Rust. There is no shell, no `tar`, no globbing and no
//! platform-specific script.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The maps a build cannot regenerate by itself.
pub const REQUIRED: &[&str] = &[
    "derived_by_form.bin.gz",
    "derived_nominal.bin.gz",
    "derived_modifiers.bin.gz",
    "pronoun_forms.bin.gz",
    "degree_forms.bin.gz",
    "adverb_possessive.bin.gz",
    "oi_plural_lemmas.bin.gz",
];

/// The remote to use unless `MAPS_REMOTE` overrides it.
pub fn remote() -> String {
    std::env::var("MAPS_REMOTE").unwrap_or_else(|_| "origin".to_string())
}

/// The repository root, from the current directory.
pub fn repo_root() -> Result<PathBuf, String> {
    let output = git(Path::new("."), &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(output.trim()))
}

/// The required maps that are missing from `<root>/maps`.
pub fn missing(root: &Path) -> Vec<&'static str> {
    REQUIRED
        .iter()
        .copied()
        .filter(|name| !root.join("maps").join(name).exists())
        .collect()
}

/// Fetch the `maps` branch and unpack its `.bin.gz` files into `maps/`.
///
/// Returns the number of blobs written. Existing files are replaced only
/// after the fetch succeeds, so a failed fetch leaves the working maps intact.
pub fn fetch(root: &Path, remote: &str) -> Result<usize, String> {
    let refname = format!("refs/remotes/{remote}/maps");
    // A build run over a flaky link should not fail on one reset; retry the
    // network step a couple of times before giving up.
    let mut last = None;
    for attempt in 0..3 {
        match git(root, &["fetch", "--force", remote, &format!("maps:{refname}")]) {
            Ok(_) => {
                last = None;
                break;
            }
            Err(err) => {
                last = Some(err);
                if attempt < 2 {
                    std::thread::sleep(std::time::Duration::from_millis(750));
                }
            }
        }
    }
    if let Some(err) = last {
        return Err(err);
    }

    let listing = git(root, &["ls-tree", "-r", "--name-only", &refname])?;
    let mut blobs: Vec<(String, String)> = Vec::new();
    for line in listing.lines() {
        let Some(name) = line.strip_prefix("maps/") else {
            continue;
        };
        if name.ends_with(".bin.gz") {
            blobs.push((name.to_string(), format!("{refname}:maps/{name}")));
        }
    }
    if blobs.is_empty() {
        return Err(format!(
            "{refname} contains no maps/*.bin.gz (was it published?)"
        ));
    }

    let dir = root.join("maps");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("create {}: {e}", dir.display()))?;
    for (name, object) in &blobs {
        let bytes = git_bytes(root, &["show", object])?;
        let path = dir.join(name);
        std::fs::write(&path, bytes)
            .map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(blobs.len())
}

/// Replace the `maps` branch with the current `maps/*.bin.gz`.
///
/// The new branch is a single parentless commit. Returns that commit id.
pub fn publish(root: &Path, remote: &str) -> Result<String, String> {
    let dir = root.join("maps");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".bin.gz"))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("no maps/*.bin.gz in {}", dir.display()));
    }

    let index = std::env::temp_dir().join(format!(
        "torille-maps-index-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&index);
    let index_env = [("GIT_INDEX_FILE", index.as_os_str())];

    let result = (|| {
        git_env(root, &["read-tree", "--empty"], &index_env)?;
        for path in &files {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            let rel = rel.to_str().ok_or("map path is not UTF-8")?;
            git_env(root, &["add", "-f", "--", rel], &index_env)?;
        }
        let readme = dir.join("README.md");
        if readme.exists() {
            git_env(root, &["add", "-f", "--", "maps/README.md"], &index_env)?;
        }
        let tree = git_env(root, &["write-tree"], &index_env)?;
        let tree = tree.trim();
        let commit = git(
            root,
            &["commit-tree", tree, "-m", "Current binary maps"],
        )?;
        let commit = commit.trim().to_string();
        // Push the commit by id first; only move the local ref if the remote
        // accepted it, so a failed network push leaves the branch untouched.
        git(
            root,
            &[
                "push",
                "--force",
                remote,
                &format!("{commit}:refs/heads/maps"),
            ],
        )?;
        git(root, &["update-ref", "refs/heads/maps", &commit])?;
        git(
            root,
            &[
                "fetch",
                "--force",
                remote,
                &format!("maps:refs/remotes/{remote}/maps"),
            ],
        )?;
        Ok(commit)
    })();
    let _ = std::fs::remove_file(&index);
    result
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    git_env(root, args, &[])
}

fn git_env(
    root: &Path,
    args: &[&str],
    env: &[(&str, &std::ffi::OsStr)],
) -> Result<String, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or(""),
            stderr.trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("git output is not UTF-8: {e}"))
}

fn git_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(args);
    let output = command
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or(""),
            stderr.trim()
        ));
    }
    Ok(output.stdout)
}
