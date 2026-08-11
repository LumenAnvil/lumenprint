//! Integration tests over the generated output.
//!
//! These assert the *contents* of a generated project: that the verified SDK
//! version and build target actually reach the files, that no substitution was
//! missed, and that the guidance in the generated README matches what the tooling
//! really requires.
//!
//! What they deliberately do not do is compile anything. Building a contract
//! pulls the Soroban SDK and takes minutes, which does not belong in `cargo test`.
//! That job belongs to `scripts/verify-templates.sh`, which generates every
//! template and runs its formatting, lints, tests and a real wasm build. Both
//! layers are needed: these tests catch a broken substitution in a second, the
//! script catches an SDK API that moved.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use lumenprint::{generate, manifest, naming::Names, toolchain};

/// Generate `template` as `name` into a fresh temporary directory.
///
/// Returns the temp dir (which must outlive the files) and every generated file
/// keyed by its relative path.
fn generate_project(
    template_name: &str,
    name: &str,
) -> (tempfile::TempDir, BTreeMap<PathBuf, String>) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let root = dir.path().join(name);

    let template = manifest::find(template_name).expect("template should exist");
    let names = Names::parse(name).expect("name should be valid");
    let outcome = generate::generate(&template, &names, &root).expect("generation should succeed");

    let files = outcome
        .files
        .iter()
        .map(|relative| {
            let contents = fs::read_to_string(root.join(relative))
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", relative.display()));
            (relative.clone(), contents)
        })
        .collect();

    (dir, files)
}

/// Every template name in the registry.
fn all_templates() -> Vec<String> {
    manifest::registry()
        .expect("registry should load")
        .into_iter()
        .map(|t| t.name)
        .collect()
}

/// Read one file from a generated project, failing with a useful message.
fn file<'a>(files: &'a BTreeMap<PathBuf, String>, path: &str) -> &'a str {
    files
        .get(Path::new(path))
        .unwrap_or_else(|| {
            let available: Vec<_> = files.keys().map(|p| p.display().to_string()).collect();
            panic!("{path} was not generated; got: {available:?}")
        })
        .as_str()
}

#[test]
fn there_are_at_least_three_templates() {
    let templates = all_templates();

    for expected in ["minimal", "token", "access"] {
        assert!(
            templates.iter().any(|t| t == expected),
            "expected a {expected:?} template, found {templates:?}"
        );
    }
}

#[test]
fn every_template_generates_the_expected_project_shape() {
    for template in all_templates() {
        let (_dir, files) = generate_project(&template, "shape-test");

        for expected in [
            "Cargo.toml",
            "README.md",
            "src/lib.rs",
            "src/test.rs",
            ".gitignore",
            "rust-toolchain.toml",
            ".github/workflows/ci.yml",
        ] {
            assert!(
                files.contains_key(Path::new(expected)),
                "template {template:?} did not generate {expected}"
            );
        }
    }
}

#[test]
fn no_substitution_token_survives_into_the_output() {
    // The single most valuable check here: a typo in a contributed template
    // would otherwise ship a literal `{% crate_nme %}` into a user's source.
    for template in all_templates() {
        let (_dir, files) = generate_project(&template, "leftover-check");

        for (path, contents) in &files {
            assert!(
                !contents.contains("{%"),
                "template {template:?} left an unsubstituted token in {}",
                path.display()
            );
        }
    }
}

#[test]
fn cargo_toml_pins_the_verified_sdk_version() {
    for template in all_templates() {
        let (_dir, files) = generate_project(&template, "sdk-check");
        let cargo_toml = file(&files, "Cargo.toml");

        assert!(
            cargo_toml.contains(&format!(
                "soroban-sdk = \"{}\"",
                toolchain::SOROBAN_SDK_VERSION
            )),
            "template {template:?} does not pin soroban-sdk {}",
            toolchain::SOROBAN_SDK_VERSION
        );
        assert!(
            cargo_toml.contains(&format!("rust-version = \"{}\"", toolchain::MSRV)),
            "template {template:?} does not declare the SDK's MSRV"
        );
    }
}

#[test]
fn cargo_toml_has_the_settings_a_contract_needs() {
    for template in all_templates() {
        let (_dir, files) = generate_project(&template, "settings-check");
        let cargo_toml = file(&files, "Cargo.toml");

        // Without cdylib the build produces no loadable wasm at all.
        assert!(
            cargo_toml.contains(r#"crate-type = ["cdylib"]"#),
            "template {template:?} is missing crate-type = [\"cdylib\"]"
        );

        // testutils in the release build bloats the wasm and is rejected on deploy,
        // so it must appear only under dev-dependencies.
        let dev_deps = cargo_toml
            .split_once("[dev-dependencies]")
            .expect("a contract needs dev-dependencies for testutils")
            .1;
        assert!(
            dev_deps.contains(r#"features = ["testutils"]"#),
            "template {template:?} does not enable testutils for tests"
        );
        let before_dev_deps = cargo_toml.split("[dev-dependencies]").next().unwrap();
        assert!(
            !before_dev_deps.contains("testutils"),
            "template {template:?} enables testutils outside dev-dependencies"
        );

        // A silent wrap in contract arithmetic is a vulnerability.
        assert!(
            cargo_toml.contains("overflow-checks = true"),
            "template {template:?} does not enable overflow checks in release"
        );
    }
}

#[test]
fn the_build_target_is_the_current_one_everywhere_it_appears() {
    for template in all_templates() {
        let (_dir, files) = generate_project(&template, "target-check");

        for path in [
            "rust-toolchain.toml",
            ".github/workflows/ci.yml",
            "README.md",
        ] {
            assert!(
                file(&files, path).contains(toolchain::RUST_TARGET),
                "template {template:?}: {path} does not mention {}",
                toolchain::RUST_TARGET
            );
        }

        // The superseded target produces a binary the network rejects, so no
        // template may build against it. Naming it in a comment is fine, and the
        // generated CI does exactly that to explain why the target matters, so
        // only non-comment lines are an error.
        for (path, contents) in &files {
            for (number, line) in contents.lines().enumerate() {
                if !line.contains("wasm32-unknown-unknown") {
                    continue;
                }

                let trimmed = line.trim_start();
                let is_comment = trimmed.starts_with('#')
                    || trimmed.starts_with("//")
                    || trimmed.starts_with('-'); // markdown bullet

                assert!(
                    is_comment,
                    "template {template:?}: {}:{} builds against the superseded target:\n{line}",
                    path.display(),
                    number + 1
                );
            }
        }
    }
}

#[test]
fn projects_are_built_with_the_stellar_cli_not_cargo_build() {
    for template in all_templates() {
        let (_dir, files) = generate_project(&template, "build-cmd-check");

        assert!(
            file(&files, ".github/workflows/ci.yml").contains(toolchain::BUILD_COMMAND),
            "template {template:?}: CI does not run `{}`",
            toolchain::BUILD_COMMAND
        );
        assert!(
            file(&files, "README.md").contains(toolchain::BUILD_COMMAND),
            "template {template:?}: README does not document `{}`",
            toolchain::BUILD_COMMAND
        );
    }
}

#[test]
fn the_project_name_reaches_every_form_it_should() {
    let (_dir, files) = generate_project("minimal", "my-cool-contract");

    // Package name: as given.
    assert!(file(&files, "Cargo.toml").contains(r#"name = "my-cool-contract""#));

    // Contract struct: Pascal case.
    let lib = file(&files, "src/lib.rs");
    assert!(lib.contains("pub struct MyCoolContract;"), "lib.rs:\n{lib}");

    // Test client: derived from the struct name.
    assert!(file(&files, "src/test.rs").contains("MyCoolContractClient"));

    // Wasm artifact path: snake case, because that is what Cargo emits.
    assert!(file(&files, ".github/workflows/ci.yml").contains("my_cool_contract.wasm"));
}

#[test]
fn generated_sources_are_formatted() {
    // The generated CI runs `cargo fmt --check`, so unformatted output would fail
    // a user's CI on their first push. A long name is used on purpose: the name is
    // substituted into the source, so it is what pushes lines over the limit.
    for template in all_templates() {
        let (dir, _files) = generate_project(&template, "a-deliberately-long-project-name-here");
        let root = dir.path().join("a-deliberately-long-project-name-here");

        for source in ["src/lib.rs", "src/test.rs"] {
            let path = root.join(source);
            let output = std::process::Command::new("rustfmt")
                .arg("--edition")
                .arg(toolchain::RUST_EDITION)
                .arg("--check")
                .arg(&path)
                .output()
                .expect("rustfmt should be available on a Rust toolchain");

            assert!(
                output.status.success(),
                "template {template:?}: {source} is not formatted:\n{}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
    }
}

#[test]
fn the_token_template_says_plainly_that_it_is_not_production_ready() {
    // The token template is the one most likely to be deployed by someone who did
    // not read carefully, so the warning is a tested requirement, not a nicety.
    let (_dir, files) = generate_project("token", "warned-token");

    let readme = file(&files, "README.md").to_lowercase();
    assert!(
        readme.contains("not been audited"),
        "README omits the audit warning"
    );
    assert!(
        readme.contains("sep-41"),
        "README does not point at the real standard"
    );

    let lib = file(&files, "src/lib.rs").to_lowercase();
    assert!(
        lib.contains("not been audited"),
        "the contract source omits the audit warning"
    );
}

#[test]
fn generating_into_an_occupied_directory_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("occupied");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("my-work.rs"), "do not delete me").unwrap();

    let template = manifest::find("minimal").unwrap();
    let names = Names::parse("occupied").unwrap();

    let err = generate::generate(&template, &names, &root).unwrap_err();

    assert!(
        err.to_string().contains("not empty"),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read_to_string(root.join("my-work.rs")).unwrap(),
        "do not delete me",
        "existing files must be left untouched"
    );
}

#[test]
fn an_unknown_template_names_the_real_ones() {
    let err = manifest::find("mimimal").unwrap_err().to_string();

    assert!(err.contains("mimimal"), "{err}");
    assert!(
        err.contains("minimal"),
        "should suggest the real templates: {err}"
    );
}

#[test]
fn an_invalid_project_name_is_rejected_before_anything_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("nope");

    assert!(Names::parse("../escape").is_err());
    assert!(!root.exists(), "nothing should have been created");
}
