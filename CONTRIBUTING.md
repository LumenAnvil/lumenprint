# Contributing to Lumenprint

Thanks for helping out. Adding a template is the easiest way in and is a
data-only change — no core logic to touch.

## Getting set up

```bash
git clone https://github.com/LumenAnvil/lumenprint
cd lumenprint
cargo build
```

To work on templates you also need the contract toolchain, because the
verification script builds real wasm:

```bash
rustup target add wasm32v1-none
cargo install --locked stellar-cli@27.1.0
```

## Before opening a pull request

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
./scripts/verify-templates.sh
```

All four must be clean. The last one takes a few minutes because it compiles the
Soroban SDK once per template; `SKIP_WASM=1` skips the Stellar CLI build if you
do not have the CLI installed locally, but CI will run it in full.

## Adding a template

This is the main extension point, and it is deliberately data-only: files plus a
manifest entry. If you find yourself editing `src/manifest.rs` or
`src/generate.rs` to add a template, something has gone wrong — please open an
issue instead, because that is a gap in the design.

### 1. Create the folder

```
src/templates/my-template/
  template.toml
  files/
    Cargo.toml.tmpl
    README.md.tmpl
    src/lib.rs.tmpl
    src/test.rs.tmpl
```

### 2. Write the manifest

```toml
description = "One line, shown by `lumenprint list`. Say what it is for."
order = 40
```

| Field | Required | Meaning |
| ----- | -------- | ------- |
| `description` | yes | One line shown in listings |
| `order` | no | Sort position; ties fall back to name order. Existing templates use 10, 20, 30 |
| `base` | no | Set `false` to opt out of the shared `_base` layer. Rarely correct — override individual files instead |

Unknown fields are rejected, so a typo fails loudly rather than being ignored.

### 3. Copy the shared files for free

`.gitignore`, `rust-toolchain.toml` and the CI workflow come from
`src/templates/_base/` automatically. Do not duplicate them. To change one for
your template, put a file at the same path in your own `files/` and it wins.

Folders starting with `_` are layers, not templates: they never appear in
`lumenprint list`.

### 4. Follow the path conventions

| In the repository | Generated as |
| ----------------- | ------------ |
| `Cargo.toml.tmpl` | `Cargo.toml` |
| `src/lib.rs.tmpl` | `src/lib.rs` |
| `_gitignore` | `.gitignore` |
| `_github/workflows/ci.yml` | `.github/workflows/ci.yml` |

The `.tmpl` suffix keeps a nested `Cargo.toml` from looking like a real package
to Cargo. The leading `_` keeps a template's `.gitignore` or `.github/` from
applying to this repository. Both are stripped on generation.

### 5. Use tokens, never hard-coded versions

Templates use `{% token %}` — see the
[token table in the README](README.md#substitution-tokens).

**Never hard-code an SDK version, target or command in a template.** They live in
[`src/toolchain.rs`](src/toolchain.rs) so that one edit updates every template. A
hard-coded version is the single most likely way for a template to rot without
anyone noticing.

An unknown or misspelled token is an error rather than a silent pass-through.

### 6. Verify it

```bash
./scripts/verify-templates.sh my-template
```

This generates the template and runs `cargo fmt --check`, `cargo clippy -D
warnings`, `cargo test` and `stellar contract build` against it — the same checks
its own CI workflow will run. A template must pass all of them to be merged.

You do not need to register the template anywhere or edit the script; both
discover templates automatically.

## What makes a good template

Templates are read by people learning Soroban, so they are teaching material as
much as code.

- **It must compile and its tests must pass.** Non-negotiable.
- **Explain the non-obvious in comments.** Why `require_auth` rather than
  comparing an address, why TTL has to be extended, why `testutils` is a
  dev-dependency. A beginner cannot tell which lines are load-bearing.
- **Write tests that would fail if the logic were wrong.** For access control
  this means `env.mock_auths(...)`, not `env.mock_all_auths()` — the latter makes
  every `require_auth` succeed, so such a test passes even with the guard
  deleted. See the `access` template.
- **Keep it small.** A template is a starting point, not a framework. If it needs
  conditionals to cover several cases, it should be several templates.
- **Say what it is not.** If a template resembles something people deploy for
  real, say plainly what it lacks. The `token` template states in both its README
  and its source that it is unaudited and not SEP-41 compliant.

## Updating the toolchain versions

When a new `soroban-sdk` is released:

1. Verify the new version against an official source — [crates.io](https://crates.io/crates/soroban-sdk)
   for the version and MSRV, the [Stellar setup guide](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup)
   for the target and CLI version. Do not rely on memory.
2. Update [`src/toolchain.rs`](src/toolchain.rs), including `VERIFIED_ON`.
3. Update the toolchain table in [README.md](README.md) and the pinned
   `STELLAR_CLI_VERSION` in [.github/workflows/ci.yml](.github/workflows/ci.yml).
4. Run `./scripts/verify-templates.sh` in full. An SDK bump is exactly when a
   deprecation turns into a CI failure, since the generated CI runs
   `clippy -D warnings`.

## Code style

- `cargo fmt` and `cargo clippy -D warnings` clean.
- Small, focused modules. Each file in `src/` has one job.
- Document *why*, not *what*. Explain the constraint that made the code look the
  way it does.
- Errors should name the file and suggest the fix. A contributor who mistypes a
  token should be told which tokens exist.

## Reporting problems

A generated project that does not compile is the most serious kind of bug here.
Please include the template name, the exact command, `rustc --version` and
`stellar --version`, and the full error.

## License

Contributions are licensed under Apache-2.0, matching the project.
