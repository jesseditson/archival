#!/bin/bash

set -e

cd $(dirname "$0")

cargo install action-validator

action-validator action.yml
action-validator .github/workflows/rust-ci.yml
