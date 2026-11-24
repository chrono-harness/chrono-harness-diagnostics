#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p target
scratch=$(mktemp -d "$(pwd -P)/target/check.XXXXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
export TMPDIR="$scratch"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$scratch/cargo}"
cargo fmt --check --manifest-path crates/diagnostics/Cargo.toml
cargo fmt --check --manifest-path crates/diagnostics-tests/Cargo.toml
product=crates/diagnostics/Cargo.toml
cargo build --locked --message-format=json-render-diagnostics --manifest-path "$product" > "$scratch/build.jsonl"
cargo test --locked --manifest-path crates/diagnostics-tests/Cargo.toml

coverage_failure() {
  echo "E_VERUS_COVERAGE: $*" >&2
  exit 1
}

verifier=$(command -v "${VERUS:-verus}") || coverage_failure "verifier not found"

prove() {
  chrono-host-bootstrap run-compiler-proof "$scratch" "$1" "$verifier" "$2" ||
    coverage_failure "$1: verifier execution failed"
}

admit_proof() {
  local label=$1
  shift
  chrono-host-bootstrap collect-compiler-proof "$scratch" "$label" "$verifier" \
    0.2026.04.24.f8e1704 f8e17045c4ac17a02e566b8997070f2eecc255be "$@" ||
    coverage_failure "$label: required proof result was not admitted"
}

# Read the exact library selected by Cargo in the build of the tested executable.
metadata=$(jq -ers --arg manifest "$(pwd -P)/$product" '
  [.[] | select(.reason == "compiler-artifact" and .manifest_path == $manifest
                 and .target.kind == ["lib"]) | .filenames[] | select(endswith(".rmeta"))]
  | if length == 1 then .[0] else error("expected the product library") end
' "$scratch/build.jsonl") || coverage_failure "native library was not identified"
filename=${metadata##*/}
stem=${filename#lib}
stem=${metadata%/*}/${stem%.rmeta}
chrono-host-bootstrap check-compiler-sources native "$stem" \
  src/verified_error.rs src/error.rs src/verified_rolling.rs src/rolling.rs src/verified_runtime.rs src/runtime.rs || coverage_failure "native library differs from required sources"

prove verify.diagnostics-error crates/diagnostics/src/verified_error.rs
prove verify.diagnostics-rolling crates/diagnostics/src/verified_rolling.rs
prove verify.diagnostics-runtime crates/diagnostics/src/verified_runtime.rs
prove verify.diagnostics-error-tests crates/diagnostics-tests/tests/verified_error.rs
prove verify.diagnostics-rolling-tests crates/diagnostics-tests/tests/verified_rolling.rs
prove verify.diagnostics-runtime-tests crates/diagnostics-tests/tests/verified_runtime.rs

admit_proof verify.diagnostics-error crates/diagnostics/src/verified_error.rs
admit_proof verify.diagnostics-rolling crates/diagnostics/src/verified_rolling.rs \
  crates/diagnostics/src/rolling.rs
admit_proof verify.diagnostics-runtime crates/diagnostics/src/verified_runtime.rs \
  crates/diagnostics/src/runtime.rs
admit_proof verify.diagnostics-error-tests crates/diagnostics-tests/tests/verified_error.rs \
  crates/diagnostics-tests/tests/../../diagnostics/src/verified_error.rs
admit_proof verify.diagnostics-rolling-tests crates/diagnostics-tests/tests/verified_rolling.rs \
  crates/diagnostics-tests/tests/../../diagnostics/src/verified_rolling.rs \
  crates/diagnostics-tests/tests/../../diagnostics/src/rolling.rs \
  crates/diagnostics-tests/tests/rolling.rs
admit_proof verify.diagnostics-runtime-tests crates/diagnostics-tests/tests/verified_runtime.rs \
  crates/diagnostics-tests/tests/../../diagnostics/src/verified_runtime.rs \
  crates/diagnostics-tests/tests/../../diagnostics/src/runtime.rs
