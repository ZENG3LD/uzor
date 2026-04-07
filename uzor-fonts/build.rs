fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set by cargo");
    let dest = std::path::Path::new(&out_dir).join("NotoColorEmoji.ttf");

    // Skip download if already cached (size > 1 MB).
    if dest.exists() {
        let size = dest.metadata().map(|m| m.len()).unwrap_or(0);
        if size > 1_048_576 {
            return;
        }
    }

    const URL: &str =
        "https://github.com/ZENG3LD/uzor/releases/download/fonts-v1/NotoColorEmoji.ttf";

    // Use curl -L to follow GitHub's 302 redirect to release-assets.githubusercontent.com.
    let status = std::process::Command::new("curl")
        .arg("-sSL")   // silent, show errors, follow redirects
        .arg("--fail") // treat HTTP 4xx/5xx as errors
        .arg("-o")
        .arg(&dest)
        .arg(URL)
        .status()
        .expect("build.rs: failed to spawn curl — ensure curl is installed and on PATH");

    if !status.success() {
        panic!(
            "build.rs: curl failed to download NotoColorEmoji.ttf from {URL} (exit: {status:?})"
        );
    }

    // Sanity-check: NotoColorEmoji is ~10 MB; reject tiny error pages.
    let downloaded_size = std::fs::metadata(&dest)
        .expect("build.rs: downloaded file missing after curl succeeded")
        .len();

    if downloaded_size < 1_000_000 {
        panic!(
            "build.rs: downloaded NotoColorEmoji.ttf is suspiciously small ({downloaded_size} bytes) \
             — likely an error page from {URL}"
        );
    }
}
