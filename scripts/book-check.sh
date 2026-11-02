#!/usr/bin/env bash
# Builds the book and checks local links, heading anchors and assets offline.
# GitHub Pages serves the book under `/spire/`, so a temporary directory mirrors
# that prefix for Lychee.
set -euo pipefail

cd "$(dirname "$0")/.."

mdbook build book

book_root=$(mktemp -d)
trap 'rm -rf "$book_root"' EXIT
ln -s "$PWD/book/book" "$book_root/spire"

lychee --offline --include-fragments --index-files index.html --root-dir "$book_root" 'book/book/**/*.html'
