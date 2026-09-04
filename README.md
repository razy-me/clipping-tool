<div align="center">

# clipping-tool

### Capture moments at the speed of play.

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-4c1?style=flat-square" alt="License MIT"></a>
  <img src="https://img.shields.io/badge/Node.js-%3E%3D18.0.0-339933?style=flat-square&logo=node.js&logoColor=white" alt="Node.js >=18.0.0">
  <img src="https://img.shields.io/badge/Rust-1.70%2B-DEA584?style=flat-square&logo=rust&logoColor=white" alt="Rust 1.70+">
  <img src="https://img.shields.io/badge/Tests-425%20Passing-2ea44f?style=flat-square&logo=githubactions&logoColor=white" alt="Tests 425 Passing">
  <img src="https://img.shields.io/badge/Status-Beta%20%7C%20WIP-orange?style=flat-square" alt="Status Beta | WIP">
  <img src="https://img.shields.io/badge/Export-MP4%20%7C%20WebM%20%7C%20Audio-0078D6?style=flat-square" alt="Export Formate">
</p>

<p align="center">
  <b>CLIPPING TOOL</b> ist ein leichtgewichtiges, blitzschnelles Windows-Screen- und Game-Capture-Tool für Gamer, Streamer und Content Creator.<br>
  Nimm flüssiges Gameplay und Desktop-Aktivitäten mit minimaler CPU/GPU-Last auf – angetrieben von <b>Tauri 2</b>, <b>Rust</b> und Hardwarebeschleunigung (<b>WGC</b>, <b>NVENC/AMF/QSV</b>) inklusive nativem Low-Latency Multi-Source Audio.
</p>

<p align="center">
  <a href="#-highlights--features">Features</a> •
  <a href="#-wichtiger-hinweis-work-in-progress">Wichtiger Status</a> •
  <a href="#-erste-schritte--getting-started">Quickstart</a> •
  <a href="#%EF%B8%8F-tech-stack--architektur">Architektur</a> •
  <a href="#-production-build-erstellen">Build</a> •
  <a href="#-lizenz">Lizenz</a>
</p>

---

</div>

> [!WARNING]
> ### ⚠️ Wichtiger Hinweis: Work in Progress
> **Dieses Tool befindet sich derzeit noch in der aktiven Entwicklung und funktioniert noch nicht zu 100% fehlerfrei.**  
> Es können unerwartete Bugs, Abstürze, Audio-/Video-Synchronisationsprobleme oder Randfall-Fehler auftreten. Feedback und Issues sind jederzeit herzlich willkommen!

---

## 🌟 Highlights & Features

| Feature | Beschreibung |
| :--- | :--- |
| ⚡ **Leicht & Resourcenschonend** | Nahezu null Idle-Last dank Tauri 2 und nativer Rust-Engine |
| 🎮 **Game- & Desktop-Capture** | Flüssige Aufnahme via Windows Graphics Capture (`WGC`) & Hardwarebeschleunigung |
| 🔊 **Multi-Source Audio** | Low-Latency Audio-Engine (WASAPI / CPAL) für Systemsound, Mikrofon & App-Audio |
| ✂️ **In-App Editor & Overlay** | Integriertes Trimming-Tool und interaktives In-Game-Overlay |
| 🚀 **Auto-Tuning** | Automatische Erkennung der GPU- und Encoder-Fähigkeiten für optimale Settings |
| ⌨️ **Hotkeys & Gamepads** | Globale Tastenkombinationen und Gamepad-Tasten-Mapping für sofortiges Clippen |
| 📁 **Clip-Bibliothek** | Übersichtliche Clip-Verwaltung, verlustfreier Export und sicherer Papierkorb |

---

## 🛠️ Tech Stack & Architektur

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
- **Media Processing**: FFmpeg & FFprobe Sidecars mit Hardware-Fallback

---

## 🚀 Erste Schritte / Getting Started

### 📋 Voraussetzungen

- **Betriebssystem**: Windows 10 / 11 (64-bit)
- **Node.js**: v18+ (LTS empfohlen)
- **Rust**: Aktuelle Stable-Toolchain über [rustup.rs](https://rustup.rs/)
- **Build Tools**: Visual Studio C++ Build Tools

### 📦 Installation & Entwicklung

1. **Repository klonen**:
   ```bash
   git clone https://github.com/razy-me/clipping-tool.git
   cd clipping-tool/clip-tool
   ```

2. **Frontend-Abhängigkeiten installieren**:
   ```bash
   npm install
   ```

3. **FFmpeg Sidecars bereitstellen**:
   Stelle sicher, dass die Windows-Binärdateien von `ffmpeg` und `ffprobe` im Verzeichnis `src-tauri/bin/` mit dem passenden Target-Namen liegen:
   - `ffmpeg-x86_64-pc-windows-msvc.exe`
   - `ffprobe-x86_64-pc-windows-msvc.exe`

4. **Entwicklungsserver starten**:
   ```bash
   npm run tauri dev
   ```

---

## 🧪 Tests durchführen

Führe die Rust-Backend-Testsuite aus (Unit Tests, DSP-Tests, Hardware-Profile, Codec-Fallbacks):

```bash
cd clip-tool/src-tauri
cargo test
```

---

## 📦 Production Build erstellen

Erstelle einen optimierten Release-Build (Installer und Executable):

```bash
cd clip-tool
npm run tauri build
```

Die fertigen Binärdateien befinden sich anschließend unter:  
`clip-tool/src-tauri/target/release/`

---

## 📄 Lizenz

Dieses Projekt steht unter der [MIT-Lizenz](LICENSE).

