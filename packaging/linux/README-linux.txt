waydlp for Linux — portable archive
===================================

Run ./waydlp. Because of the waydlp.portable file, settings and history are
kept in a "data" folder next to it. Delete waydlp.portable to use the normal
XDG locations (~/.config/waydlp, ~/.local/share/waydlp) instead.

To install for your user (menu entry, ~/.local/bin/waydlp):
    ./waydlp-installer install --file ../waydlp-<version>-linux-x64.tar.gz
or simply run ./waydlp-setup for the graphical installer.

Dependencies
------------
  yt-dlp  (required) — waydlp can install a verified copy (Settings → yt-dlp & FFmpeg),
                       or use your distribution's package / pipx.
  ffmpeg  (recommended) — install with your package manager:
      Debian/Ubuntu/Mint:  sudo apt install ffmpeg
      Fedora:              sudo dnf install ffmpeg-free
      openSUSE:            sudo zypper install ffmpeg
      Arch & derivatives:  sudo pacman -S ffmpeg
  The GUI needs X11 or Wayland and Vulkan or OpenGL (Mesa) drivers.

License: GPL-3.0-or-later. Source: https://github.com/waywhywhy/waydlp-desktop-rust
