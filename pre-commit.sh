#!/bin/bash

set -e

cd $(dirname "$0")

cargo clippy --all-features --all-targets -- -D warnings
./test-bin.sh
