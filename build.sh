#!/bin/sh

set -e

git clone git@github.com:rust-lang/rust.git

cd rust

grep FIXME **/*.rs -n > ../fixmes.txt

cd ..

cargo run
