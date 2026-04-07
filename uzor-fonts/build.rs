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

    let response = ureq::get(URL).call().unwrap_or_else(|e| {
        panic!(
            "build.rs: failed to download NotoColorEmoji.ttf from {URL}\n\
             Error: {e}\n\
             Check your internet connection or create the GitHub release at the URL above."
        )
    });

    let mut bytes: Vec<u8> = Vec::new();
    std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes).unwrap_or_else(|e| {
        panic!("build.rs: failed to read response body for NotoColorEmoji.ttf: {e}")
    });

    if bytes.len() < 1_048_576 {
        panic!(
            "build.rs: downloaded NotoColorEmoji.ttf is suspiciously small ({} bytes). \
             The GitHub release at {URL} may be missing or broken.",
            bytes.len()
        );
    }

    std::fs::write(&dest, &bytes).unwrap_or_else(|e| {
        panic!(
            "build.rs: failed to write NotoColorEmoji.ttf to {}: {e}",
            dest.display()
        )
    });
}
