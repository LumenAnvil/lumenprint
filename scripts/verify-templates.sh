#!/usr/bin/env bash
#
# Generate every template and prove it is a working project.
#
# A template that does not compile is worse than no template at all: it costs a
# newcomer an afternoon before they conclude the tool is broken. Templates rot
# quietly — an SDK release deprecates an API, a version moves on — and nothing in
# `cargo test` at the top level would notice, because Lumenprint's own tests only
# check that the right text was written to the right files.
#
# This script closes that gap by treating each generated project as a project:
# formatting, lints, tests, and a real wasm build with the Stellar CLI.
#
# It discovers templates from `lumenprint list`, so a newly contributed template
# is covered automatically with no edit here.
#
# Usage:
#   scripts/verify-templates.sh              # every template, including the wasm build
#   scripts/verify-templates.sh minimal      # only the named templates
#   SKIP_WASM=1 scripts/verify-templates.sh  # skip the Stellar CLI build
#
# Environment:
#   SKIP_WASM=1   Skip `stellar contract build`. Useful locally when the Stellar
#                 CLI is not installed; CI must never set it, since the wasm build
#                 is the check that matters most.
#   KEEP_TMP=1    Leave the generated projects on disk for inspection.

set -euo pipefail

readonly REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly SKIP_WASM="${SKIP_WASM:-0}"
readonly KEEP_TMP="${KEEP_TMP:-0}"

# Generated projects are built here. A shared target directory across templates
# would be faster, but each project pins its own toolchain file, so keeping them
# separate avoids surprising interactions.
WORKDIR="$(mktemp -d -t lumenprint-verify-XXXXXX)"

cleanup() {
  if [[ "$KEEP_TMP" == "1" ]]; then
    echo "Generated projects left in $WORKDIR"
  else
    rm -rf "$WORKDIR"
  fi
}
trap cleanup EXIT

log()  { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
step() { printf '    \033[2m%s\033[0m\n' "$*"; }
fail() { printf '\033[31mFAIL: %s\033[0m\n' "$*" >&2; }

require() {
  command -v "$1" >/dev/null 2>&1 || {
    fail "$1 is required but not installed"
    exit 1
  }
}

require cargo
require rustfmt

if [[ "$SKIP_WASM" != "1" ]]; then
  require stellar
fi

# Build once, in release: the binary is invoked once per template and a debug
# build makes that noticeably slower.
log "Building lumenprint"
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml"
readonly LUMENPRINT="$REPO_ROOT/target/release/lumenprint"

# Templates named on the command line, or all of them.
if [[ $# -gt 0 ]]; then
  templates=("$@")
else
  # `list` prints "<name>  <description>"; the name is the first field.
  mapfile -t templates < <("$LUMENPRINT" list | awk '{print $1}')
fi

if [[ ${#templates[@]} -eq 0 ]]; then
  fail "no templates found"
  exit 1
fi

echo "Verifying: ${templates[*]}"

failed=()

for template in "${templates[@]}"; do
  # A long name is used deliberately. The project name is substituted into the
  # source, so a long one produces long lines; generating with a short name would
  # not prove the formatting check survives a realistic worst case.
  project="verify-${template}-contract"
  project_dir="$WORKDIR/$template/$project"

  log "Template: $template"

  if ! "$LUMENPRINT" new "$project" --template "$template" --path "$project_dir" >/dev/null; then
    fail "$template: generation failed"
    failed+=("$template")
    continue
  fi

  ok=1
  pushd "$project_dir" >/dev/null

  # Exactly the checks the generated project's own CI workflow runs, in the same
  # order, so a green run here means a green run there.
  step "cargo fmt --check"
  cargo fmt --all -- --check || { fail "$template: generated sources are not formatted"; ok=0; }

  if [[ $ok -eq 1 ]]; then
    step "cargo clippy -D warnings"
    cargo clippy --all-targets -- -D warnings || { fail "$template: clippy found problems"; ok=0; }
  fi

  if [[ $ok -eq 1 ]]; then
    step "cargo test"
    cargo test || { fail "$template: generated tests failed"; ok=0; }
  fi

  if [[ $ok -eq 1 && "$SKIP_WASM" != "1" ]]; then
    step "stellar contract build"
    if stellar contract build; then
      # Succeeding is not quite enough: confirm a wasm actually landed where the
      # generated README and CI tell people to look for it.
      if ! compgen -G "target/*/release/*.wasm" >/dev/null; then
        fail "$template: build reported success but produced no wasm"
        ok=0
      fi
    else
      fail "$template: stellar contract build failed"
      ok=0
    fi
  fi

  popd >/dev/null

  if [[ $ok -eq 1 ]]; then
    printf '\033[32m    PASS: %s\033[0m\n' "$template"
  else
    failed+=("$template")
  fi
done

echo
if [[ ${#failed[@]} -gt 0 ]]; then
  fail "${#failed[@]} template(s) failed: ${failed[*]}"
  exit 1
fi

printf '\033[32mAll %d template(s) verified.\033[0m\n' "${#templates[@]}"
