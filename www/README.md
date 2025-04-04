# kdr Web Example

`index.html` contains the minimal example to get kdr working on your web site.

## Building

You need to build kdr with `wasm-back`. Check for the `wasm.sh` script to see how it is build.
By default, the script has `--target web`, which means no JavaScript bundler.
You can simply remove that argument so it can build for your typical NodeJS project.
