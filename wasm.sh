#/bin/usr/env sh
SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"

wasm-pack build --target web -d $SCRIPT_DIR/www/pkg $@ kdr
