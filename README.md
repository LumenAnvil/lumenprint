# Lumenprint

Scaffolding CLI for [Stellar](https://stellar.org)/[Soroban](https://developers.stellar.org/docs/build/smart-contracts)
smart contracts. It generates a correct, documented, test-ready contract project
from a template, so you start from something that already builds instead of
assembling boilerplate.

```console
$ lumenprint new my-contract
Created my-contract from the `minimal` template (7 files)
  soroban-sdk 27.0.5, target wasm32v1-none

Next steps:
  cd my-contract
  cargo test                 # run the generated tests
  stellar contract build     # build the wasm
```

Lumenprint is an ordinary native Rust CLI. It *generates* contract crates; it is
not itself a contract and is never compiled to wasm.

## Why

Starting a Soroban contract means getting a pile of unrelated details right at
once, and each one fails in a way that is hard to diagnose if you are new:

| Detail | What goes wrong |
| ------ | --------------- |
| Build target | `wasm32-unknown-unknown` still exists and still "works"; it produces a binary the network rejects. The correct target is `wasm32v1-none`. |
| Build command | `cargo build` produces wasm that is missing contract metadata. Contracts are built with `stellar contract build`. |
| `crate-type` | Without `["cdylib"]` the build silently produces no loadable contract. |
| `testutils` | Needed to write tests, but enabling it outside `[dev-dependencies]` bloats the wasm and is rejected on deploy. |
| Release profile | Without `overflow-checks = true`, arithmetic wraps silently — in a contract, that is a vulnerability, not a rounding error. |
| Storage TTL | Contract data is archived unless its time-to-live is extended, so a contract that worked in tests starts failing weeks after deploy. |

Every generated project has all of these right, and says why in comments you can
read rather than cargo-cult.

## Toolchain

Generated projects target the versions below. Each was checked against an
official source on **2026-08-11**, and they live in one place — [`src/toolchain.rs`](src/toolchain.rs) —
so a bump propagates to every template at once.

| Component | Version | Source |
| --------- | ------- | ------ |
| `soroban-sdk` | `27.0.5` | [crates.io](https://crates.io/crates/soroban-sdk) |
| Build target | `wasm32v1-none` | [Stellar setup guide](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup) |
| Rust | `1.91`+ | the `rust-version` published with `soroban-sdk` |
| Rust edition | `2021` | the `edition` published with `soroban-sdk` |
| Stellar CLI | `27.1.0` | [Stellar setup guide](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup) |
| Build command | `stellar contract build` | [Stellar docs](https://developers.stellar.org/docs/build/smart-contracts/getting-started/hello-world) |

Note that the `wasm32v1-none` target itself needs only Rust 1.84+, but the SDK
declares a higher floor, so 1.91 is the binding constraint.

These are not aspirational. CI generates every template and builds it with the
real Stellar CLI, so if a version drifts, the build goes red. See
[template verification](#template-verification).

## Prerequisites

Lumenprint itself needs only a Rust toolchain. To build the projects it
generates, you also need the contract target and the Stellar CLI:

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# The contract build target
rustup target add wasm32v1-none

# The Stellar CLI
cargo install --locked stellar-cli@27.1.0
```

Generated projects include a `rust-toolchain.toml` that pins the toolchain and
target, so `rustup` usually installs the target for you on first build.

## Install

```bash
cargo install --git https://github.com/LumenAnvil/lumenprint
```

Or from a clone:

```bash
git clone https://github.com/LumenAnvil/lumenprint
cd lumenprint
cargo install --path .
```

## Usage

### `lumenprint list`

```console
$ lumenprint list
minimal  A hello + counter contract. Start here: shows contract structure, instance storage, and tests.
access   An owner/admin pattern: constructor-set admin, require_auth guards, and admin transfer.
token    A fungible-token skeleton: balances, transfer, mint and burn. A starting point, not an audited token.
```

### `lumenprint new`

```bash
lumenprint new my-contract                      # uses the `minimal` template
lumenprint new my-vault --template access       # pick a template
lumenprint new my-token --template token --path ./contracts/token
```

| Flag | Meaning |
| ---- | ------- |
| `-t`, `--template <NAME>` | Template to generate from. Defaults to `minimal`. |
| `--path <DIR>` | Directory to create. Defaults to `./<name>`. |

The project name is used three ways: as given for the Cargo package
(`my-contract`), snake case for the crate and wasm file (`my_contract`), and
Pascal case for the contract struct (`MyContract`).

An existing non-empty directory is refused rather than merged, so a mistyped
command cannot overwrite your work.

## Templates

Every template generates a complete project — `Cargo.toml`, the contract,
tests using the SDK test utilities, a README, `.gitignore`, `rust-toolchain.toml`,
and a CI workflow that runs a real contract build.

### `minimal`

A greeting and a persistent counter. The place to start: it shows contract
structure, instance storage and TTL extension, and the test client, without any
other concepts in the way.

### `access`

The owner/admin pattern most contracts need before anything else. The admin is
set in `__constructor`, which runs atomically with deployment — a separate
`initialize` anyone may call first is a well-known way to lose a contract in the
gap between deploying and configuring it. Privileged functions are guarded with
`require_auth`.

Its tests use `env.mock_auths(...)` rather than `env.mock_all_auths()` for the
access-control cases, which matters: `mock_all_auths` makes every `require_auth`
succeed, so a test using it would still pass with the guard deleted.

### `token`

A fungible-token skeleton: balances in persistent storage, transfers, admin-only
minting, burning, supply accounting and events.

**It is a starting point, not a token you should deploy.** It has not been
audited and does not implement the full
[SEP-41 token interface](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
(`soroban_sdk::token::TokenInterface`) — no allowances, no `transfer_from` — so
wallets and DEXes will not know how to use it. To wrap an existing Stellar asset,
use the built-in [Stellar Asset Contract](https://developers.stellar.org/docs/tokens/stellar-asset-contract)
instead. The generated README and the contract source both say so.

## Adding a template

Adding a template is a **data-only change**: files plus a manifest entry. No core
logic to edit, no registry to update.

```
src/templates/my-template/
  template.toml          # description and sort order
  files/                 # everything here is generated into the new project
    Cargo.toml.tmpl
    src/lib.rs.tmpl
    src/test.rs.tmpl
    README.md.tmpl
```

`template.toml`:

```toml
description = "One line, shown by `lumenprint list`."
order = 40
```

Shared files — `.gitignore`, `rust-toolchain.toml`, the CI workflow — come from
`src/templates/_base/` automatically. Override any of them by placing a file at
the same path in your template. Folders starting with `_` are layers, not
templates.

Two path conventions apply to everything under `files/`:

| In the repository | Generated as | Why |
| ----------------- | ------------ | --- |
| `Cargo.toml.tmpl` | `Cargo.toml` | Keeps a nested `Cargo.toml` from looking like a real package to Cargo |
| `_gitignore` | `.gitignore` | A real `.gitignore` under `src/` would apply to this repository |
| `_github/workflows/ci.yml` | `.github/workflows/ci.yml` | Same reason |

### Substitution tokens

Templates use `{% token %}`. The delimiter is `{% %}` rather than `{{ }}` because
generated projects ship a GitHub Actions workflow, and Actions expressions are
spelled `${{ matrix.os }}` — a `{{ }}` delimiter would collide with them.

| Token | Example |
| ----- | ------- |
| `{% project_name %}` | `my-contract` |
| `{% crate_name %}` | `my_contract` |
| `{% contract_name %}` | `MyContract` |
| `{% sdk_version %}` | `27.0.5` |
| `{% rust_target %}` | `wasm32v1-none` |
| `{% msrv %}` | `1.91` |
| `{% rust_edition %}` | `2021` |
| `{% stellar_cli_version %}` | `27.1.0` |
| `{% build_command %}` | `stellar contract build` |
| `{% verified_on %}` | `2026-08-11` |

An unknown or misspelled token is an error, not a silent pass-through — a typo
cannot ship a literal `{% crate_nme %}` into someone's source file.

Never hard-code a version in a template. Use the tokens, so that bumping
[`src/toolchain.rs`](src/toolchain.rs) updates everything at once.

### Verify it

```bash
./scripts/verify-templates.sh my-template
```

This must pass before a template is merged. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Template verification

A template that does not compile is worse than no template: it costs a newcomer
an afternoon before they conclude the tool is broken. Templates rot quietly — an
SDK release deprecates an API, a version moves on — and Lumenprint's own unit
tests cannot catch it, because they only check that the right text reached the
right files.

So there are two layers:

| Layer | Runs | Catches |
| ----- | ---- | ------- |
| `cargo test` | seconds | Missed substitutions, wrong SDK version, missing files, unformatted output |
| `scripts/verify-templates.sh` | minutes | An SDK API that moved, a deprecation, a build that no longer produces wasm |

The script generates every template and treats it as a real project — `cargo fmt
--check`, `cargo clippy -D warnings`, `cargo test`, and `stellar contract build`
— then confirms a wasm actually landed. It discovers templates from `lumenprint
list`, so a new template is covered with no edit to the script.

```bash
./scripts/verify-templates.sh              # every template
./scripts/verify-templates.sh minimal      # just one
SKIP_WASM=1 ./scripts/verify-templates.sh  # skip the Stellar CLI build
KEEP_TMP=1 ./scripts/verify-templates.sh   # leave the generated projects on disk
```

CI runs it on every push and on a weekly schedule, because templates can break
without anyone touching this repository.

## How it works

Templates are embedded into the binary at compile time with `include_dir`, so
generating a project needs no network, no git and no files installed alongside
the executable. Substitution is plain token replacement — no conditionals, no
loops — which keeps every template readable as the file it will produce. A
template that needs branching should be a separate template.

Generated Rust sources are run through `rustfmt`. That is not cosmetic: the
project name is substituted into the source, so a longer name makes longer lines,
and no fixed template text can be correctly formatted for every possible name.
Since the generated CI runs `cargo fmt --check`, unformatted output would fail a
user's CI on their first push.

```
src/
  main.rs        clap CLI entry point
  lib.rs         library surface, so tests drive the same code the binary does
  toolchain.rs   verified versions and targets — the single source of truth
  manifest.rs    template discovery and registry
  generate.rs    substitution and file writing
  naming.rs      name validation and case conversion
  templates/     embedded template files, one folder per template
tests/           generation tests over the produced files
scripts/         template verification
```

## Non-goals

Deliberately out of scope, with clean extension points left where they would go:

- **Deployment and invocation.** That is the Stellar CLI's job, and it does it
  well. Generated READMEs show the commands.
- **Remote template fetching.** Embedding is what makes the binary self-contained
  and offline-capable.
- **A TUI.** Two subcommands do not need one.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Adding a template is the easiest way in,
and is a data-only change.

## License

Apache-2.0. See [LICENSE](LICENSE).
