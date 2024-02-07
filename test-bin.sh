#!/bin/bash

set -e

RUST_LOG=debug cargo test --features=binary $@ -- --test-threads=1

rm -rf target/file-system-tests
rm -rf target/binary-tests
