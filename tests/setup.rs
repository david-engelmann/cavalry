//! The setup invariants, re-expressed as Rust tests under
//! CV-45.
//!
//! The Python pins read the tree for the stub obligations, the
//! canonical agents block, the README round count, and the
//! dashes the copy bar forbids. The port re-expresses them in
//! the same forms cavalry has always used, with the dash scan
//! walking the suffix set of the ported tree, the source
//! suffix of this crate standing where the Python suffix stood.

mod common;

use common::root;
use sha2::Digest;

const CANONICAL_PREFIX_SHA256: &str =
    "63d2f04005dc8eb111d3efca1be55c0911f9ad46bc7cf3c74d9f9908f4a86f11";
const EXPECTED_OBLIGATIONS: [&str; 5] = [
    "CV-S1. Every recorded rescue carries who intervened, what they did, and which run it touched, and a record missing any of those fields is rejected rather than stored incomplete.",
    "CV-S2. Every rescue is classified into a named class, and a record that cannot be classified is held in an explicit unclassified state that reports itself rather than passing silently.",
    "CV-S3. Every class with at least one recorded rescue carries a drafted hook or rule meant to retire that class, and a class with rescues and no draft is a defect the tests must catch.",
    "CV-S4. Cavalry records human rescues only, and an automatic rescue performed by prod is never written into the cavalry record as a human rescue.",
    "CV-S5. Every fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.",
];
const EXPECTED_PREFIX: &str = "CV";
const TEXT_SUFFIXES: [&str; 6] = ["md", "yml", "yaml", "rs", "toml", "txt"];
const SKIP_PARTS: [&str; 8] = [
    ".git",
    "target",
    ".pytest_cache",
    "__pycache__",
    ".venv",
    "venv",
    "node_modules",
    ".tox",
];

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

/// Whether a line opens a stub obligation, and the obligation
/// prefix and number it carries when it does, reading the same
/// shape the Python pattern read.
fn stub_obligation(line: &str) -> Option<(String, u32)> {
    let bytes = line.as_bytes();
    if bytes.len() < 6 {
        return None;
    }
    if !bytes[0].is_ascii_uppercase() || !bytes[1].is_ascii_uppercase() {
        return None;
    }
    if &line[2..4] != "-S" {
        return None;
    }
    let digit = bytes[4];
    if !digit.is_ascii_digit() || bytes[5] != b'.' {
        return None;
    }
    Some((line[..2].to_string(), (digit - b'0') as u32))
}

#[test]
fn contract_stub_obligations_exist() {
    let text = std::fs::read_to_string(root().join("CONTRACT.md")).unwrap();
    let mut found: Vec<(String, u32, String)> = Vec::new();
    for line in text.lines() {
        if let Some((prefix, number)) = stub_obligation(line) {
            found.push((prefix, number, line.to_string()));
        }
    }
    assert!(
        found.len() >= 5,
        "expected at least 5 stub obligations, found {}",
        found.len()
    );
    let mut numbers: Vec<u32> = found.iter().map(|(_, number, _)| *number).collect();
    numbers.sort_unstable();
    let texts: Vec<&str> = found.iter().map(|(_, _, line)| line.as_str()).collect();
    assert_eq!(
        texts,
        EXPECTED_OBLIGATIONS.to_vec(),
        "stub obligation text has changed without a contract amendment"
    );
    assert_eq!(
        numbers,
        vec![1, 2, 3, 4, 5],
        "stub obligations must be exactly S1 to S5"
    );
    for (prefix, _, line) in &found {
        assert_eq!(prefix, EXPECTED_PREFIX, "obligation is not a CV obligation");
        let words = line.split_whitespace().count();
        assert!(words >= 20, "obligation too thin at {} words", words);
    }
}

#[test]
fn agents_canonical_block_present() {
    let text = std::fs::read_to_string(root().join("AGENTS.md")).unwrap();
    assert!(text.contains("repo annex below this line"));
    let prefix = text.split("## Repo annex").next().unwrap();
    let digest = sha256_hex(prefix.as_bytes());
    assert_eq!(
        digest, CANONICAL_PREFIX_SHA256,
        "canonical block above the annex line has drifted"
    );
    let annex = text.split("## Repo annex").nth(1).unwrap();
    assert!(annex.contains("CONTRACT.md"));
}

#[test]
fn readme_status_reports_rounds() {
    let text = std::fs::read_to_string(root().join("README.md")).unwrap();
    // The pin is cavalry's own form, the one its Round 1
    // closure restored. The count phrase stands somewhere in
    // the README and is not the head of a longer number.
    let needle = "Closed rounds 5 of 100";
    let mut found = false;
    let mut search = text.as_str();
    while let Some(at) = search.find(needle) {
        let after = &search[at + needle.len()..];
        if !after.starts_with(|ch: char| ch.is_ascii_digit()) {
            found = true;
            break;
        }
        search = &search[at + needle.len()..];
    }
    assert!(
        found,
        "README status must report closed rounds 5 of 100 exactly"
    );
}

#[test]
fn no_em_or_en_dash_in_text_files() {
    let mut bad = Vec::new();
    let mut stack = vec![root()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("tree readable") {
            let path = entry.expect("dir entry").path();
            let relative = path.strip_prefix(root()).unwrap().to_path_buf();
            if relative
                .components()
                .any(|part| SKIP_PARTS.contains(&part.as_os_str().to_string_lossy().as_ref()))
            {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let suffix = path
                .extension()
                .map(|ext| ext.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !TEXT_SUFFIXES.contains(&suffix.as_str()) {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("text file readable");
            if text.contains('\u{2014}') || text.contains('\u{2013}') {
                bad.push(relative.to_string_lossy().into_owned());
            }
        }
    }
    assert!(bad.is_empty(), "em or en dash found in {:?}", bad);
}
