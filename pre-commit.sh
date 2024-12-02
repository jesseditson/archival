#!/bin/bash

set -e

cd $(dirname "$0")

cargo fmt -- --check --color always
cargo clippy --all-features --all-targets -- --no-deps -D warnings
./test.sh
