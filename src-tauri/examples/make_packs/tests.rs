//! Determinism tests for the pack generator (feature 05: AC5).
//!
//! Interface assumed from the plan, in `main.rs` (included there as
//! `#[cfg(test)] mod tests;`):
//!
//! ```ignore
//! fn generate(out: &Path) -> io::Result<()>
//! ```
//!
//! It writes the four pack folders (`blue`, `brown`, `red`, `typewriter`) and
//! `LICENSE.md` into `out`, creating folders as needed, with nothing else.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::generate;

/// A temporary folder removed on drop.
struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "mks-makepacks-{}-{}-{}",
            std::process::id(),
            label,
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Temp(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Every file under `root`, keyed by its `/`-separated relative path.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn committed() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("packs")
}

fn assert_same(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>, what: &str) {
    let only_a: Vec<_> = a.keys().filter(|k| !b.contains_key(*k)).collect();
    let only_b: Vec<_> = b.keys().filter(|k| !a.contains_key(*k)).collect();
    assert!(only_a.is_empty(), "{what}: only in first: {only_a:?}");
    assert!(only_b.is_empty(), "{what}: only in second: {only_b:?}");
    let differing: Vec<_> = a
        .iter()
        .filter(|(k, v)| &b[*k] != *v)
        .map(|(k, _)| k)
        .collect();
    assert!(
        differing.is_empty(),
        "{what}: bytes differ in {differing:?}"
    );
}

#[test]
fn ac5_the_generator_writes_the_four_packs_and_the_licence() {
    let out = Temp::new("shape");
    generate(&out.0).unwrap();
    let files = tree(&out.0);
    assert!(files.contains_key("LICENSE.md"));
    for id in ["blue", "brown", "red", "typewriter"] {
        assert!(
            files.contains_key(&format!("{id}/pack.json")),
            "{id}/pack.json"
        );
        assert!(
            files
                .keys()
                .any(|k| k.starts_with(&format!("{id}/")) && k.ends_with(".wav")),
            "{id} has no wav"
        );
    }
}

#[test]
fn ac5_generating_twice_gives_byte_identical_files() {
    let (a, b) = (Temp::new("first"), Temp::new("second"));
    generate(&a.0).unwrap();
    generate(&b.0).unwrap();
    assert_same(&tree(&a.0), &tree(&b.0), "two runs");
}

#[test]
fn ac5_generating_into_an_empty_folder_matches_the_committed_packs() {
    let out = Temp::new("committed");
    generate(&out.0).unwrap();
    let committed = tree(&committed());
    assert!(!committed.is_empty(), "no committed packs");
    assert_same(&tree(&out.0), &committed, "generated vs committed");
}
