# 🎬 Clipping Tool

A high-performance, lightweight Windows game & screen clipping application built with **Tauri 2**, **Rust**, and **TypeScript / Vite**.

Capture your favorite gaming moments and desktop gameplay efficiently with low latency and minimal resource overhead using Windows Graphics Capture (WGC) and hardware acceleration.

---

## ✨ Features

- **⚡ Lightweight & Fast**: Built on Tauri 2 and Rust for near-zero idle resource usage and high performance.
- **🎮 Game & Screen Recording**: Screen and game capture powered by Windows Graphics Capture (`WGC`) and Direct3D/GDI.
- **🔊 Low-Latency Multi-Source Audio**: Native WASAPI and CPAL audio engine supporting system sound, microphone, and application audio.
- **✂️ In-App Editor & Webview Overlay**: Built-in video trimming/editing tools and an interactive in-game overlay.
- **🚀 Hardware Profile & Auto-Tuning**: Automatic detection of GPU/encoder capabilities to select optimal recording profiles.
- **⌨️ Customizable Hotkeys & Gamepad Integration**: Global shortcut support and gamepad event mapping for instantaneous clipping.
- **📁 Clip Management**: Built-in library management with lossless exports, clipboard integration, and safe trash bin operations.

---

## 🛠️ Tech Stack

- **Backend / Core Engine**: [Rust](https://www.rust-lang.org/) (Tauri 2, Tokio, Windows API / WASAPI, CPAL, Axum)
- **Frontend / UI**: [TypeScript](https://www.typescriptlang.org/), [Vite](https://vitejs.dev/)
- **Media Processing**: FFmpeg & FFprobe sidecars with hardware-accelerated encoding fallbacks

---

## 🚀 Getting Started

### Prerequisites

1. **Operating System**: Windows 10/11 (64-bit)
2. **Node.js**: LTS version installed (v18+)
3. **Rust**: Latest stable toolchain installed via [rustup](https://rustup.rs/)
4. **C++ Build Tools**: Visual Studio C++ Build Tools installed

### Installation & Development

1. **Clone the repository**:
   ```bash
   git clone https://github.com/razy-me/clipping-tool.git
   cd clipping-tool/clip-tool
   ```

2. **Install frontend dependencies**:
   ```bash
   npm install
   ```

3. **FFmpeg Sidecars**:
   Ensure `ffmpeg` and `ffprobe` Windows binaries are available under `src-tauri/bin/` with their appropriate target names:
   - `ffmpeg-x86_64-pc-windows-msvc.exe`
   - `ffprobe-x86_64-pc-windows-msvc.exe`

4. **Run in development mode**:
   ```bash
   npm run tauri dev
   ```

---

## 🧪 Testing

Run backend Rust test suites (unit tests, DSP tests, hardware profiles, codec fallback):
```bash
cd clip-tool/src-tauri
cargo test
```

---

## 📦 Building for Production

To create an optimized production release installer/executable:

```bash
cd clip-tool
npm run tauri build
```

The compiled binaries will be output into `clip-tool/src-tauri/target/release/`.

---

## 📄 License

This project is licensed under the [MIT License](LICENSE) (or your project's default license).
