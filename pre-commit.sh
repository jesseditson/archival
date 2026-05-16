#!/bin/bash

set -e

cd $(dirname "$0")

# Verify package.json version matches Cargo.toml version
CARGO_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
NPM_VERSION=$(node -e "console.log(require('./package.json').version)")
if [ "$CARGO_VERSION" != "$NPM_VERSION" ]; then
    echo "Error: Cargo.toml version ($CARGO_VERSION) does not match package.json version ($NPM_VERSION)"
    exit 1
fi

cargo fmt -- --check --color always
cargo clippy --all-features --all-targets -- --no-deps -D warnings
./test.sh

./validate-actions.sh
