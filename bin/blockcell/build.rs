use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

const ROOT_INPUTS: &[&str] = &[
    "index.html",
    "package.json",
    "package-lock.json",
    "postcss.config.mjs",
    "scripts/source-hashes.mjs",
    "tailwind.config.ts",
    "tsconfig.app.json",
    "tsconfig.json",
    "tsconfig.node.json",
    "vite.config.ts",
];
const INPUT_DIRECTORIES: &[&str] = &["public", "src"];

fn collect_files(root: &Path, relative_dir: &Path, output: &mut BTreeSet<PathBuf>) {
    let directory = root.join(relative_dir);
    let entries = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}"));
        let relative_path = relative_dir.join(entry.file_name());
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!("failed to inspect {}: {error}", relative_path.display())
        });
        if file_type.is_dir() {
            collect_files(root, &relative_path, output);
        } else if file_type.is_file() {
            output.insert(relative_path);
        }
    }
}

fn sha256(path: &Path) -> String {
    let bytes = fs::read(path)
        .unwrap_or_else(|error| panic!("failed to read WebUI source {}: {error}", path.display()));
    format!("{:x}", Sha256::digest(bytes))
}

fn parse_manifest(contents: &str) -> BTreeMap<PathBuf, String> {
    contents
        .lines()
        .map(|line| {
            let (digest, relative) = line
                .split_once("  ")
                .unwrap_or_else(|| panic!("invalid WebUI source hash line: {line}"));
            assert!(
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "invalid SHA-256 digest in WebUI source hash manifest"
            );
            let path = PathBuf::from(relative);
            assert!(
                !path.is_absolute()
                    && path
                        .components()
                        .all(|component| matches!(component, Component::Normal(_))),
                "unsafe path in WebUI source hash manifest: {relative}"
            );
            (path, digest.to_ascii_lowercase())
        })
        .collect()
}

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let webui_root = manifest_dir.join("../../webui");
    let hash_manifest = webui_root.join("dist/source-hashes.txt");
    let contents = fs::read_to_string(&hash_manifest).unwrap_or_else(|_| {
        panic!(
            "embedded WebUI is not built; run `npm ci && npm test && npm run build` in {}",
            webui_root.display()
        )
    });
    let recorded = parse_manifest(&contents);

    let mut expected_paths: BTreeSet<PathBuf> = ROOT_INPUTS.iter().map(PathBuf::from).collect();
    for directory in INPUT_DIRECTORIES {
        collect_files(&webui_root, Path::new(directory), &mut expected_paths);
    }

    let recorded_paths = recorded.keys().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        recorded_paths,
        expected_paths,
        "embedded WebUI source set changed; run `npm run build` in {}",
        webui_root.display()
    );

    for relative_path in expected_paths {
        let source_path = webui_root.join(&relative_path);
        println!("cargo:rerun-if-changed={}", source_path.display());
        let actual = sha256(&source_path);
        let expected = recorded
            .get(&relative_path)
            .expect("source hash entry must exist");
        assert_eq!(
            &actual,
            expected,
            "embedded WebUI is stale because {} changed; run `npm run build` in {}",
            relative_path.display(),
            webui_root.display()
        );
    }
    println!("cargo:rerun-if-changed={}", hash_manifest.display());
}
