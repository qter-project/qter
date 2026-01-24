#!/bin/bash

cd "$(dirname "$0")";

if [ "$1" = "tsc-watch" ]; then
    tsc --watch;
elif [ "$1" = "wasm-pack" ]; then
    wasm-pack build --target web --out-dir dist --dev;
elif [ "$1" = "cp-static" ]; then
    cp -R static/* dist;
elif [ "$1" = "caddy-serve" ]; then
    caddy file-server -root dist;
elif [ "$1" = "clean" ]; then
    rm -r dist node_modules;
    rm -rf tree-sitter-q;
else
    mkdir -p dist;
    npm install;
    git clone https://github.com/qter-project/tree-sitter-q;
    npx tree-sitter build --wasm tree-sitter-q -o dist/tree-sitter-qter_q.wasm;
    cp tree-sitter-q/queries/highlights.scm dist/highlights.scm;
    cp node_modules/web-tree-sitter/web-tree-sitter.{js,js.map,d.ts,d.ts.map,wasm,wasm.map} dist;
    wasm-pack build --target web --out-dir dist --dev;
    rm dist/.gitignore;
    cp -R static/* dist;
    tsc;
fi