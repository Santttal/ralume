# Ralume

Simple Linux screen recorder with independent speaker/microphone audio toggles,
built on a Wayland-first stack: xdg-desktop-portal ScreenCast + PipeWire +
GStreamer + GTK4/libadwaita.

![status](https://img.shields.io/badge/status-MVP-blue) ![license](https://img.shields.io/badge/license-GPL--3.0-green) ![platform](https://img.shields.io/badge/platform-Linux-lightgrey)

## Features

- 🎥 **Screen capture** via xdg-desktop-portal (GNOME, KDE, wlroots, Hyprland)
- 🔊 **System audio** from the default sink monitor (no mic required)
- 🎙 **Microphone** independent toggle
- 🎚 **Mixed or separate audio tracks** — choose in Preferences
- ⚡ **Hardware encoding** — NVIDIA NVENC, Intel VAAPI, falls back to x264enc
  software encoder automatically
- ⚙️ **Preferences** — FPS, bitrate, audio mode, output folder, encoder hint
- 🗂 **Crash-safe MKV** — file plays even if the app is killed mid-recording
- 🔴 **Recording indicator** — pulsing red dot in the header bar and
  elapsed timer

## Screenshots

> Add screenshots here after first release.

## Quick start (pre-built binary)

1. Download the latest release:
   ```
   wget https://github.com/Santttal/ralume/releases/latest/download/ralume-linux-x86_64.tar.gz
   tar xzf ralume-linux-x86_64.tar.gz
   cd ralume-*
   ./scripts/install.sh
   ```
2. Launch from the app menu or run `ralume` in a terminal.

Pre-built binaries target Ubuntu 22.04+ / Fedora 38+ / Arch (current) with the
following runtime dependencies installed:

```
gtk4 >= 4.6, libadwaita-1 >= 1.1,
gstreamer1.0-plugins-base/good/bad/ugly,
gstreamer1.0-pipewire (or gstreamer1.0-plugins-rs-nvcodec for NVENC),
pipewire, pipewire-pulseaudio (or classic pulseaudio),
xdg-desktop-portal and your compositor's backend.
```

On Ubuntu 22.04:
```
sudo apt install libgtk-4-1 libadwaita-1-0 \
  gstreamer1.0-plugins-{base,good,bad,ugly} gstreamer1.0-pipewire \
  pipewire xdg-desktop-portal xdg-desktop-portal-gnome
```

## Build from source

Requires **Rust stable**, **GStreamer**, **GTK4**, **libadwaita** dev packages.

```
# Clone
git clone https://github.com/Santttal/ralume.git
cd screen-recorder

# Install runtime dev deps (Ubuntu 22.04 example)
sudo apt install libgtk-4-dev libadwaita-1-dev libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev

# Build
cargo build --release

# Run
./scripts/run.sh
```

## Project structure

```
src/
  main.rs                — bootstrap, actions, channels, tokio runtime
  ui/{window,preferences,events,style}.rs
  portal/{screencast,state,shortcuts}.rs
  recorder/{pipeline,encoders,audio,output}.rs
  config/settings.rs
data/{style.css,*.desktop,icons/}
scripts/{run.sh,install.sh,package-release.sh}
docs/                    — architecture, implementation log, task breakdown
```

See `docs/architecture.md` for a detailed walkthrough.

## Hardware acceleration

At startup, available H.264 encoders are detected via
`gst::ElementFactory::find`. Priority order:

1. `nvh264enc` (NVIDIA NVENC) — best quality/CPU trade-off on Turing+
2. `vah264enc` / `vaapih264enc` (Intel / AMD VAAPI)
3. `qsvh264enc` (Intel Quick Sync)
4. `x264enc` (software, universal fallback)

You can force a specific backend in **Preferences → Video → Encoder**.

## Known limitations

- Only H.264 is fully wired; the UI exposes H.265/VP9/AV1 as placeholders.
- Only MKV muxing is active. Extensions MP4/WebM chosen in Preferences change
  the filename only; real remux via `ffmpeg -c copy` is on the roadmap.
- Global hotkeys are stored but not registered (needs `GlobalShortcuts`
  portal; on some compositors it is unavailable).
- Localization is Russian-only for now; `en.po` coming before first Flatpak.

## Roadmap

- [x] MVP: screen + audio, portal flow, Preferences
- [x] Hardware encoding (NVENC / VAAPI)
- [ ] MKV → MP4 remux via ffmpeg
- [ ] Global shortcuts
- [ ] Flatpak package on Flathub
- [ ] Localization (en)

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).

## Credits

Inspired by [Kooha](https://github.com/SeaDve/Kooha) and the
[xdg-desktop-portal](https://flatpak.github.io/xdg-desktop-portal/) team.

## Why "Ralume"?

A short, abstract coined name evoking *ra* (as in raster, ray) + *lume*
(light, luminance). Chosen for search uniqueness — the app name should not
collide with existing video/recording products.
