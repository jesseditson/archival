#!/bin/bash

set -e

# First just smoke the regular tests with a wasm target, as a compilation
# failure will hang the wasm-pack tests
RUST_BACKTRACE=1 cargo build --target=wasm32-unknown-unknown --no-default-features --features=wasm-fs
RUST_BACKTRACE=1 cargo test --no-default-features --features=wasm-fs -- --test-threads=1

wasm-pack test --firefox --headless --no-default-features --features=wasm-fs
