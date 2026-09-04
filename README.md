<div align="center">

# clipping-tool

### Capture moments at the speed of play.

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-4c1?style=flat-square" alt="License MIT"></a>
  <img src="https://img.shields.io/badge/Node.js-%3E%3D18.0.0-339933?style=flat-square&logo=node.js&logoColor=white" alt="Node.js >=18.0.0">
  <img src="https://img.shields.io/badge/Rust-1.70%2B-DEA584?style=flat-square&logo=rust&logoColor=white" alt="Rust 1.70+">
  <img src="https://img.shields.io/badge/Tests-425%20Passing-2ea44f?style=flat-square&logo=githubactions&logoColor=white" alt="Tests 425 Passing">
  <img src="https://img.shields.io/badge/Status-Beta%20%7C%20WIP-orange?style=flat-square" alt="Status Beta | WIP">
  <img src="https://img.shields.io/badge/Export-MP4%20%7C%20WebM%20%7C%20Audio-0078D6?style=flat-square" alt="Export Formats">
</p>

<p align="center">
  <b>CLIPPING TOOL</b> is a lightweight, blazing-fast Windows screen and game capture application for gamers, streamers, and content creators.<br>
  Record smooth gameplay and desktop activity with minimal CPU and GPU overhead — powered by <b>Tauri 2</b>, <b>Rust</b>, hardware acceleration (<b>WGC</b>, <b>NVENC/AMF/QSV</b>), and a native low-latency multi-source audio engine.
</p>

<p align="center">
  <a href="#-highlights--features">Features</a> •
  <a href="#-work-in-progress--known-status">Status</a> •
  <a href="#-getting-started">Quickstart</a> •
  <a href="#%EF%B8%8F-tech-stack--architecture">Architecture</a> •
  <a href="#-building-for-production">Build</a> •
  <a href="#-license">License</a>
</p>

---

</div>

> [!WARNING]
> ### ⚠️ Work in Progress / Beta Notice
> **This tool is under active development and is not yet 100% bug-free.**  
> You may encounter unexpected bugs, crashes, audio/video synchronization glitches, or unhandled edge cases. Feedback, bug reports, and contributions are always welcome!

---

## 🌟 Highlights & Features

| Feature | Description |
| :--- | :--- |
| ⚡ **Lightweight & Resource-Friendly** | Near-zero idle resource usage powered by Tauri 2 and a native Rust engine |
| 🎮 **Game & Desktop Capture** | Smooth recording via Windows Graphics Capture (`WGC`) & hardware acceleration |
| 🔊 **Multi-Source Audio** | Low-latency audio engine (WASAPI / CPAL) supporting system audio, mic, & app streams |
| ✂️ **In-App Editor & Overlay** | Built-in video trimming suite and an interactive in-game overlay |
| 🚀 **Hardware Auto-Tuning** | Automated detection of GPU and encoder capabilities to select optimal settings |
| ⌨️ **Hotkeys & Gamepads** | Global keybindings and gamepad trigger mapping for instantaneous clipping |
| 📁 **Clip Library** | Organized clip management, lossless exports, and safe trash bin operations |

---

## 🛠️ Tech Stack & Architecture

```
┌────────────────────────────────────────────────────────┐
│                   Frontend (UI)                        │
│          TypeScript • Vite • Tailwind CSS              │
└──────────────────────────┬─────────────────────────────┘
                           │  Tauri IPC (Events & Commands)
┌──────────────────────────▼─────────────────────────────┐
│                 Core Engine (Rust)                     │
│  Windows API (WGC) • WASAPI / CPAL Audio • Tokio Runtime│
└──────────────────────────┬─────────────────────────────┘
                           │  Sidecar Executables
┌──────────────────────────▼─────────────────────────────┐
│               Media Processing (FFmpeg)                │
│       Hardware-accelerated Encoders (NVENC, AMF, QSV)  │
└────────────────────────────────────────────────────────┘
```

- **Backend / Core Engine**: [Rust](https://www.rust-lang.org/) (Tauri 2, Tokio, Windows API, WASAPI, CPAL, Axum)
- **Frontend / UI**: [TypeScript](https://www.typescriptlang.org/), [Vite](https://vitejs.dev/)
- **Media Processing**: FFmpeg & FFprobe sidecars with hardware-accelerated fallback

---

## 🚀 Getting Started

### 📋 Prerequisites

- **Operating System**: Windows 10 / 11 (64-bit)
- **Node.js**: v18+ (LTS recommended)
- **Rust**: Latest stable toolchain via [rustup.rs](https://rustup.rs/)
- **Build Tools**: Visual Studio C++ Build Tools

### 📦 Installation & Development

1. **Clone the repository**:
   ```bash
   git clone https://github.com/razy-me/clipping-tool.git
   cd clipping-tool/clip-tool
   ```

2. **Install frontend dependencies**:
   ```bash
   npm install
   ```

3. **Provide FFmpeg Sidecars**:
   Ensure `ffmpeg` and `ffprobe` Windows binaries are placed inside `src-tauri/bin/` with their proper target names:
   - `ffmpeg-x86_64-pc-windows-msvc.exe`
   - `ffprobe-x86_64-pc-windows-msvc.exe`

4. **Launch development server**:
   ```bash
   npm run tauri dev
   ```

---

## 🧪 Running Tests

Execute the Rust backend test suite (unit tests, DSP tests, hardware profiles, codec fallback):

```bash
cd clip-tool/src-tauri
cargo test
```

---

## 📦 Building for Production

Create an optimized release build (installer and standalone executable):

```bash
cd clip-tool
npm run tauri build
```

Compiled binaries will be located at:  
`clip-tool/src-tauri/target/release/`

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
