#!/bin/sh

# Note that there does not appear to currently be a distributed binary available
# for capnp-rust (see: https://github.com/capnproto/capnproto-rust/issues/182).
# To run this, clone capnp-rust (git@github.com:capnproto/capnproto-rust.git)
# and then from inside that repo run:
# cargo install --path capnpc

# We could use the capnpc dependency to install this of course, but we
# don't want to make this a build-time dependency of archival, as the schemas
# don't change often and you shouldn't need capnpc to use this library - the
# protos are exposed under a flag and are used to serialize/deserialize into
# rust types with backwards compatibility.

capnp compile -orust:src/proto/generated --src-prefix=proto proto/archival.capnp proto/util.capnp

cargo fmt
