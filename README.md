# 🌸 言の葉 (kotonoha)

> **Blazing-fast CLI Japanese $i+1$ sentence miner & card generator for passive immersion.**

[![Rust](https://img.shields.io/badge/Language-Rust-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Sudachi](https://img.shields.io/badge/NLP-sudachi.rs-pink.svg)](https://github.com/WorksApplications/sudachi.rs)

`kotonoha` is a high-performance terminal utility that scans Japanese anime subtitle files (`.srt` / `.ass`) and video files (`.mkv` / `.mp4`), extracts $i+1$ candidate sentences (sentences containing **exactly one unknown vocabulary word**), renders an interactive TUI card preview with definitions & audio snippets, and saves mined cards to a local SQLite database or Anki.

---

## ⚡ Key Features

- **⚡ Sub-Millisecond Morphological Analysis**: Powered by the official [`sudachi.rs`](https://github.com/WorksApplications/sudachi.rs) engine from WorksApplications.
- **🎯 Smart $i+1$ Candidate Filtering**: Automatically identifies sentences containing 1 unknown content word and ranks them by JPDB frequency and sentence length.
- **🖥️ 100% Terminal TUI**: Fully keyboard-driven interactive card review using [`inquire`](https://crates.io/crates/inquire) (arrow keys `↑`/`↓` and `Enter`).
- **🎧 Non-Blocking Preview Audio**: Background audio playback via `mpv` daemon—zero terminal freeze or input locking.
- **🎬 Single-Pass Media Extraction**: Extracts precise audio snippets (`.mp3`) and screenshot thumbnails (`.jpg`) via `ffmpeg`.
- **📦 Embedded Zero-Dependency Database**: Local SQLite storage (`~/.config/kotonoha/kotonoha.db`) for known vocabulary, ignored words, and mined card history.

---

## 🚀 Quick Start

### 🪄 One-Liner Magic (Recommended)

Paste this into your terminal to install `kotonoha` automatically:

```bash
curl -sSL https://raw.githubusercontent.com/Praveensenpai/kotonoha/main/install.sh | bash
```

<br>

### 🛠️ Building From Source

```bash
git clone https://github.com/Praveensenpai/kotonoha.git
cd kotonoha
cargo build --release
install -Dm 755 target/release/kotonoha ~/.local/bin/kotonoha
install -Dm 644 completions/kotonoha.bash ~/.local/share/bash-completion/completions/kotonoha
```

---

## 📖 Usage

### 1. Interactive TUI File Picker
Simply run `kotonoha` without arguments to launch the interactive terminal file picker:
```bash
kotonoha
```

### 2. Direct File Argument
Pass a subtitle or video file path directly:
```bash
kotonoha "Ore wo Suki nano wa Omae dake ka yo - 01.mkv"
```

---

## 🛠️ Architecture & Tech Stack

| Component | Technology | Description |
| :--- | :--- | :--- |
| **Language** | Rust 2021 | Native speed & memory safety |
| **Japanese NLP** | [`sudachi.rs`](https://github.com/WorksApplications/sudachi.rs) | WorksApplications Japanese tokenizer & POS analyzer |
| **Database** | SQLite (`rusqlite`) | Local storage for known words & mined cards |
| **TUI Engine** | `inquire` & `console` | Styled terminal prompts and card boxes |
| **Media Engine** | `ffmpeg` & `mpv` | Headless audio extraction and background preview |

---

## 📜 License

Distributed under the [MIT License](LICENSE).
