//! Fingerprints the crate's sources so the cached parsed library (see
//! `src/data/snapshot.rs`) is discarded whenever the code that produced it
//! changes.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::{fs, io};

fn hash_dir(dir: &Path, hasher: &mut DefaultHasher) -> io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            hash_dir(&path, hasher)?;
        } else {
            path.file_name().hash(hasher);
            fs::read(&path)?.hash(hasher);
        }
    }
    Ok(())
}

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.lock");
    let mut hasher = DefaultHasher::new();
    hash_dir(Path::new("src"), &mut hasher).expect("hashing src/");
    fs::read("Cargo.lock").unwrap_or_default().hash(&mut hasher);
    println!("cargo:rustc-env=SOURCE_FINGERPRINT={:x}", hasher.finish());
}
