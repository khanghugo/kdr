#/bin/usr/env sh
cargo run --package rest-api-server --target x86_64-unknown-linux-gnu --features cors --release -- $@
