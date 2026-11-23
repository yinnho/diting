// The facade's only build job (#58): bake the source revision. The snapshot
// lives with diting_js's build.rs, the Chrome target with diting_env's.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Re-embed the commit when HEAD moves (branch switches change HEAD;
    // same-branch commits move refs/heads/<branch>). Ask git where those
    // live: this crate left the repo root in the #58 move, so the old
    // `.git/HEAD` spelled from here pointed at nothing — and a watched file
    // that never exists reads as dirty, so this crate recompiled on every
    // cargo call. A tree without .git (the 86quan rsync) has nothing to
    // watch; the commit arrives through DITING_BUILD_COMMIT below.
    for rel in ["HEAD", "refs/heads"] {
        if let Some(path) = git_path(rel) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    // Source revision baked into the binary — /health reports it so a caller
    // can verify doc/tag/binary/source are the same commit without sniffing
    // anything (0.3.0 tmall report P2). "unknown" for git-less builds.
    // DITING_BUILD_COMMIT overrides git: the 86quan deploy rsyncs
    // without .git (by design — the server tree is a build input, not a
    // checkout), so the deploy step passes the commit explicitly and /health
    // stops reporting "unknown".
    let commit = std::env::var("DITING_BUILD_COMMIT")
        .or_else(|_| std::env::var("AGINXBROWSER_BUILD_COMMIT"))
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });
    println!("cargo:rustc-env=DITING_BUILD_COMMIT={commit}");
}

/// Where git keeps `rel` (`HEAD`, `refs/heads`, …), as a path cargo can
/// watch from this crate's directory: git answers relative to the current
/// directory for the main checkout, and absolute for a linked worktree
/// (its HEAD sits in the main repo's `.git/worktrees/`). `None` when this
/// is not a git checkout, or git has no such file (a detached shallow
/// clone carries no `refs/heads`).
fn git_path(rel: &str) -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", rel])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    path.exists().then_some(path)
}
