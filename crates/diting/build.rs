// The facade's only build job (#58): bake the source revision. The snapshot
// lives with diting_js's build.rs, the Chrome target with diting_env's.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Re-embed the commit when HEAD moves. Branch switches change HEAD;
    // same-branch commits move the exact ref HEAD names. Watching the
    // refs/heads directory is not enough: cargo does not treat the loose-ref
    // rewrite as a dependable recursive directory change, so the facade can
    // stay cached and /health reports a stale commit inside an otherwise-new
    // binary. Packed refs have no loose file to watch, so packed-refs is the
    // input there. A tree without .git (the 86quan rsync) has nothing to
    // watch; the commit arrives through DITING_BUILD_COMMIT below.
    if let Some(path) = git_path("HEAD") {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    // The reflog is the robust same-branch signal: a commit updates it both
    // when the branch is a loose ref and when the only ref text lives in
    // packed-refs. It is absent in some minimal/foreign checkouts, so it is
    // an additional input rather than the only one.
    if let Some(path) = git_path("logs/HEAD") {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    if let Some(head_ref) = git_output(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git_path(&head_ref) {
            println!("cargo:rerun-if-changed={}", path.display());
        } else if let Some(path) = git_path("packed-refs") {
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

fn git_output(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Where git keeps `rel` (`HEAD`, `refs/heads/main`, …), as a path cargo can
/// watch from this crate's directory: git answers relative to the current
/// directory for the main checkout, and absolute for a linked worktree
/// (its HEAD sits in the main repo's `.git/worktrees/`). `None` when this
/// is not a git checkout, or git has no such file (a branch whose only ref
/// lives in `packed-refs`).
fn git_path(rel: &str) -> Option<std::path::PathBuf> {
    let text = git_output(&["rev-parse", "--git-path", rel])?;
    let path = std::path::PathBuf::from(text);
    path.exists().then_some(path)
}
