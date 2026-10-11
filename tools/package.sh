#!/bin/bash
# ORRERY packaging: single self-contained binary (font, catalog, theory tables and the
# Horizons freeze are compile-time assets). Windows port: same crate graph under MSVC
# targets; key storage switches to Credential Manager (key_store.rs documents it).
set -e
cd "$(dirname "$0")/.."
cargo build --release
mkdir -p dist
# Windows builds produce orrery.exe (launch.bat prefers dist\Orrery.exe);
# Linux/mac produce a plain `orrery` binary.
if [ -f target/release/orrery.exe ]; then
  cp target/release/orrery.exe dist/Orrery.exe
  echo "dist/Orrery.exe ready: $(du -h dist/Orrery.exe | cut -f1)"
else
  cp target/release/orrery dist/Orrery
  echo "dist/Orrery ready: $(du -h dist/Orrery | cut -f1)"
fi
echo "assets embedded: fonts/PressStart2P, catalog.json, horizons freeze, VSOP87 (static)"
