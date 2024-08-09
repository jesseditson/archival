#!/bin/bash

set -e

cd $(dirname "$0")

cargo clippy --all-features --all-targets -- --no-deps -D warnings
./test.sh
