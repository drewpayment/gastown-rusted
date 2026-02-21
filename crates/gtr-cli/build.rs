// crates/gtr-cli/build.rs
fn main() {
    // Embed git describe output (e.g. "0.1.0-3-gabc1234" or "abc1234")
    let git_version = std::process::Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().trim_start_matches('v').to_string())
        .unwrap_or_else(|| "source".to_string());

    println!("cargo:rustc-env=GIT_VERSION={git_version}");

    // Embed build date
    let build_date = std::process::Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=BUILD_DATE={build_date}");

    // Re-run if HEAD or any branch/tag ref changes.
    // packed-refs is updated by git fetch, gc, and push.
    // logs/HEAD covers new commits on the current branch.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/packed-refs");
    println!("cargo:rerun-if-changed=../../.git/refs/tags");
    println!("cargo:rerun-if-changed=../../.git/logs/HEAD");
}
