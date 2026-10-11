#!/bin/bash
# ORRERY dev environment bootstrap (Debian 12 sandbox / any Linux)
# Installs: system toolchain, Rust stable (to /opt/rust), Xvfb+scrot for headless GUI verification.
set -e
apt-get update
apt-get install -y build-essential pkg-config libxkbcommon-dev libwayland-dev libudev-dev xvfb scrot python3 \
  libxcursor1 libxi6 libxrandr2 libxkbcommon-x11-0 libasound2 libgl1-mesa-dri
if ! command -v cargo >/dev/null; then
  python3 -c "import urllib.request; urllib.request.urlretrieve('https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init','/tmp/rustup-init')"
  chmod +x /tmp/rustup-init
  RUSTUP_HOME=/opt/rust/.rustup CARGO_HOME=/opt/rust/.cargo /tmp/rustup-init -y --profile minimal --default-toolchain stable
  mkdir -p /opt/rust
  for t in cargo rustc rustup; do
    printf '#!/bin/sh\nexport RUSTUP_HOME=/opt/rust/.rustup\nexport CARGO_HOME=/opt/rust/.cargo\nexec /opt/rust/.cargo/bin/%s "$@"\n' "$t" > /usr/local/bin/$t
    chmod +x /usr/local/bin/$t
  done
fi
cargo --version && echo "ENV READY"
