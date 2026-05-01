//! Download large font assets that aren't bundled in `fonts/` (kept out of the
//! crate to avoid bloating crates.io publishes). Files land in `OUT_DIR` and
//! `lib.rs` references them via `include_bytes!(concat!(env!("OUT_DIR"), …))`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // (filename in OUT_DIR, download URL, min expected size in bytes)
    let downloads: &[(&str, &str, u64)] = &[
        (
            "NotoColorEmoji.ttf",
            "https://github.com/ZENG3LD/uzor/releases/download/fonts-v1/NotoColorEmoji.ttf",
            1_000_000,
        ),
    ];

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set by cargo");

    for (name, url, min_size) in downloads {
        let dest = std::path::Path::new(&out_dir).join(name);

        if dest.exists() {
            let size = dest.metadata().map(|m| m.len()).unwrap_or(0);
            if size >= *min_size {
                continue;
            }
        }

        let status = std::process::Command::new("curl")
            .arg("-sSL")
            .arg("--fail")
            .arg("-o")
            .arg(&dest)
            .arg(url)
            .status()
            .expect("build.rs: failed to spawn curl — ensure curl is installed and on PATH");

        if !status.success() {
            panic!(
                "build.rs: curl failed to download {name} from {url} (exit: {status:?})"
            );
        }

        let downloaded_size = std::fs::metadata(&dest)
            .expect("build.rs: downloaded file missing after curl succeeded")
            .len();

        if downloaded_size < *min_size {
            panic!(
                "build.rs: downloaded {name} is suspiciously small \
                 ({downloaded_size} bytes < expected {min_size}) — likely an error page from {url}"
            );
        }
    }
}
