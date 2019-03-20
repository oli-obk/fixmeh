#!/bin/sh

set -e

git clone git@github.com:rust-lang/rust.git --depth=1 || echo "already downloaded"

