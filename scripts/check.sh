#!/usr/bin/env bash
# Runs every check for the project: format, lint, type check, tests.
# Locally the format step rewrites files; when CI is set it only checks.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

manifest=src-tauri/Cargo.toml

for tool in node npm cargo; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "check failed: '$tool' is not on the PATH." >&2
    exit 1
  fi
done

step() {
  local name=$1
  shift
  echo
  echo "==> $name"
  if ! "$@"; then
    echo >&2
    echo "check failed at: $name" >&2
    exit 1
  fi
}

if [ -n "${CI:-}" ]; then
  step "format (UI, check only)" npx biome format .
  step "format (core, check only)" cargo fmt --manifest-path "$manifest" --check
else
  step "format (UI)" npx biome format --write .
  step "format (core)" cargo fmt --manifest-path "$manifest"
fi

step "lint (UI)" npx biome lint --error-on-warnings .
step "lint (core)" cargo clippy --manifest-path "$manifest" --all-targets -- -D warnings
step "type check (UI)" npx tsc --noEmit
step "tests (UI)" npm test
step "tests (core)" cargo test --manifest-path "$manifest"

echo
echo "All checks passed."
