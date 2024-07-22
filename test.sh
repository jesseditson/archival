#!/bin/bash

set -e

echo "---- features: binary"
RUST_LOG=debug cargo test --features=binary $@
echo "---- features: [no default], typescript"
RUST_LOG=debug cargo test --no-default-features --features=typescript $@

rm -rf target/file-system-tests
rm -rf target/binary-tests
