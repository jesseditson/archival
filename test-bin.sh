#!/bin/bash

set -e

RUST_LOG=debug cargo test --features=binary $@
RUST_LOG=debug cargo test --no-default-features $@

rm -rf target/file-system-tests
rm -rf target/binary-tests
