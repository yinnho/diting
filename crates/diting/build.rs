// The facade's only build job (#58): bake the source revision. The snapshot
// lives with diting_js's build.rs, the Chrome target with diting_env's.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Re-embed the commit when HEAD moves (branch switches change .git/HEAD;
    // same-branch commits move .git/refs/heads/<branch> — cargo watches both
    // when present). Release CI builds from a fresh checkout either way.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

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
