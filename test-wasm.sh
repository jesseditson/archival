#!/bin/bash

# First just smoke the regular tests with a wasm target, as a compilation
# failure will hang the wasm-pack tests
RUST_BACKTRACE=1 cargo test --target=wasm32-unknown-unknown --features=wasm-fs

wasm-pack test --firefox --headless
