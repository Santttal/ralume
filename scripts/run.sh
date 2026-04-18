#!/usr/bin/env bash
set -euo pipefail

export RUST_LOG="${RUST_LOG:-debug,ralume=trace}"
export GST_DEBUG="${GST_DEBUG:-2}"
export GST_DEBUG_DUMP_DOT_DIR="${GST_DEBUG_DUMP_DOT_DIR:-/tmp/ralume_dot}"
export PATH="$HOME/.cargo/bin:$PATH"

mkdir -p "$GST_DEBUG_DUMP_DOT_DIR"

cd "$(dirname "$0")/.."
exec cargo run "$@"
