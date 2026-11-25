#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
if [[ $# -eq 0 ]]; then
  /bin/sh .chrono-harness/ci/install.sh
  export PATH="$PWD/.chrono-harness/bin:$PATH"
  export CHRONO_CHECK_SOURCE=${CHRONO_CHECK_SOURCE:-local}
  exec .chrono-harness/bin/chrono-harness check
fi
if [[ $# == 1 && $1 == --native ]]; then
  exec chrono-host-bootstrap native-scratch run /bin/bash ./check.sh --native-in-directory
fi
if [[ $# -ne 1 || $1 != --native-in-directory ]]; then
  echo 'usage: ./check.sh [--native]' >&2
  exit 2
fi
scratch=${CHRONO_NATIVE_SCRATCH:?missing native check directory}
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
  src/verified_error.rs src/error.rs src/verified_rolling.rs src/rolling.rs src/verified_runtime.rs src/runtime.rs src/trace_events.rs || coverage_failure "native library differs from required sources"
