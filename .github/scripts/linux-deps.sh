#!/usr/bin/env bash
# System packages needed to build waydlp's GUI crates on Debian/Ubuntu.
set -euo pipefail
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libgl1-mesa-dev libegl1-mesa-dev \
  libdbus-1-dev rpm desktop-file-utils appstream file
