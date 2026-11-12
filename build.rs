fn main() {
    println!("cargo:rerun-if-changed=locales");

    let profile = std::env::var("PROFILE").unwrap();
    let target_dir = std::path::PathBuf::from("target").join(profile);
    let dest = target_dir.join("locales");

    let _ = std::fs::create_dir_all(&dest);
    if let Ok(entries) = std::fs::read_dir("locales") {
        for entry in entries.flatten() {
            let src = entry.path();
            if src.is_file() {
                let dst = dest.join(src.file_name().unwrap());
                let _ = std::fs::copy(&src, &dst);
            }
        }
    }
}