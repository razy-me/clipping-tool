<div align="center">

# clipping-tool

### Capture moments at the speed of play.

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-4c1?style=flat-square" alt="License MIT"></a>
  <img src="https://img.shields.io/badge/Node.js-%3E%3D18-339933?style=flat-square&logo=node.js&logoColor=white" alt="Node.js >=18">
  <img src="https://img.shields.io/badge/Rust-1.70%2B-DEA584?style=flat-square&logo=rust&logoColor=white" alt="Rust 1.70+">
  <img src="https://img.shields.io/badge/Tests-425%20Passing-2ea44f?style=flat-square&logo=githubactions&logoColor=white" alt="Tests 425 Passing">
  <img src="https://img.shields.io/badge/Status-Beta%20%7C%20WIP-orange?style=flat-square" alt="Status Beta | WIP">
  <img src="https://img.shields.io/badge/Export-MP4%20%7C%20WebM-0078D6?style=flat-square" alt="Export Formats">
</p>

<p align="center">
  A lightweight, fast Windows screen and game capture app built with <b>Tauri 2</b>, <b>Rust</b>, and <b>TypeScript</b>.<br>
  Low resource footprint, hardware-accelerated encoding (NVENC / AMF / QSV), and low-latency multi-source audio.
</p>

---

</div>

> [!WARNING]
> **Work in Progress**: This tool is currently in active development and is not yet 100% bug-free.

---

## Features

- **⚡ Lightweight**: Minimal CPU/GPU idle usage via native Rust & Tauri 2.
- **🎮 Smooth Capture**: Hardware-accelerated desktop & game capture using Windows Graphics Capture (`WGC`).
- **🔊 Multi-Source Audio**: Low-latency WASAPI/CPAL engine for system sound, mic, and apps.
- **✂️ In-App Trimming & Overlay**: Quick clipping, built-in trimmer, and an in-game overlay.
- **⌨️ Hotkeys & Gamepad**: Trigger clips instantly with customizable shortcuts or controller buttons.

---

## Quickstart

### Prerequisites
- Windows 10/11 (64-bit)
- Node.js (v18+) & Rust (stable via [rustup](https://rustup.rs/))
- FFmpeg sidecars in `clip-tool/src-tauri/bin/` (`ffmpeg-x86_64-pc-windows-msvc.exe`, `ffprobe-x86_64-pc-windows-msvc.exe`)

### Development
```bash
git clone https://github.com/razy-me/clipping-tool.git
cd clipping-tool/clip-tool
npm install
npm run tauri dev
```

### Build & Test
```bash
# Run tests (425 tests)
cd src-tauri && cargo test

# Build release installer
npm run tauri build
```

---

## License

[MIT License](LICENSE)
