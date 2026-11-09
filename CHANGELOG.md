# Changelog

All notable changes to Lumenprint are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- An `events` template, demonstrating `#[contractevent]` structs and the
  publish/read pattern, with its own tests.

### Fixed

- CI: install `stellar-cli`'s system build dependencies
  (`libdbus-1-dev`, `libudev-dev`, `pkg-config`) so the template-verification
  job builds.
- `cargo doc` under `-D warnings`: dropped intra-doc links from public docs to
  private items.

## [0.1.0]

### Added

- Initial release: a scaffolding CLI that generates known-good Soroban contract
  projects from embedded templates.
- `lumenprint new <name> [--template <name>] [--path <dir>]` and
  `lumenprint list`.
- A data-only template extension point: a folder under `src/templates/` with a
  `template.toml` and a `files/` tree appears automatically, with a shared
  `_base` layer for files every project needs.
- `{% token %}` substitution for project names and verified toolchain versions,
  chosen so it never collides with GitHub Actions `${{ }}` expressions.
- `minimal` and `events` templates, plus rustfmt over generated sources so a new
  project passes its own `cargo fmt --check`.

[Unreleased]: https://github.com/LumenAnvil/lumenprint/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/LumenAnvil/lumenprint/releases/tag/v0.1.0
