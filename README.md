# 🌸 言の葉 (kotonoha) ✨

> **Blazing-fast Japanese $i+1$ sentence miner, naturalness quality scorer & media card generator for passive immersion.**

[![Latest Release](https://img.shields.io/github/v/release/Praveensenpai/kotonoha?style=for-the-badge&color=cba6f7&logo=github)](https://github.com/Praveensenpai/kotonoha/releases)
[![Rust Edition](https://img.shields.io/badge/Rust-2021%20Edition-DEA584?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)](https://github.com/Praveensenpai/kotonoha)
[![NLP Engine](https://img.shields.io/badge/NLP-sudachi.rs-f38ba8?style=for-the-badge)](https://github.com/WorksApplications/sudachi.rs)
[![License](https://img.shields.io/badge/License-MIT-89b4fa?style=for-the-badge)](LICENSE)

<p align="center">
  <a href="#-quick-start">⚡ Quick Install</a> •
  <a href="#-key-features">✨ Key Features</a> •
  <a href="#-pipeline--architecture">🔄 Architecture Flow</a> •
  <a href="#-usage">📖 Usage</a> •
  <a href="#%EF%B8%8F-cli-reference">🛠️ CLI Reference</a> •
  <a href="#-koto-bundles">📦 .koto Bundles</a>
</p>

---

> [!TIP]
> **Zero API Bottlenecks · 100% Offline-Capable · Instant Grammar Scoring**  
> `kotonoha` eliminates fragmented, incomplete dialogue cards using an on-device Japanese **`QualityScorer`**. No cloud latency, no manual sentence pruning—just clean, contextual Anki cards with synchronized audio & screenshots in seconds.

---

## 🔄 Pipeline & Architecture

```text
  ┌────────────────────────┐      ┌────────────────────────┐
  │  Anime Video (.mkv)    │      │  Subtitles (.srt/.ass) │
  └───────────┬────────────┘      └───────────┬────────────┘
              │                               │
              ▼                               ▼
       [ FFmpeg Demux ]               [ Sudachi.rs Tokenizer ]
              │                               │
              │                      (Morphological Analysis)
              │                               │
              ▼                               ▼
      ┌───────────────┐              ┌────────────────────────┐
      │ Audio Snippet │              │   i+1 Candidate Filter │
      │ & Screenshots │              │  (Exactly 1 Unknown)   │
      └───────┬───────┘              └───────────┬────────────┘
              │                                  │
              │                                  ▼
              │                      ┌────────────────────────┐
              │                      │ Naturalness & Quality  │
              │                      │ Scorer (QualityScorer) │
              │                      └───────────┬────────────┘
              │                                  │
              └────────────────┬─────────────────┘
                               │
                               ▼
               ┌───────────────────────────────┐
               │    Interactive Terminal TUI   │
               │   (Ratatui / Inquire Preview) │
               └───────────────┬───────────────┘
                               │
              ┌────────────────┴────────────────┐
              ▼                                 ▼
      [ SQLite Database ]               [ AnkiConnect API ]
    (~/.local/share/kotonoha)         (Audio, Image & Cards)
```

---

## ✨ Key Features

- **🧠 Intelligent Naturalness & i+1 Quality Scorer (`v0.0.73+`)**:
  - Replaces naive shortest-character heuristics with multi-factor Japanese linguistic evaluation.
  - **Defect Gating**: Penalizes cut-off thoughts ending in dangling connectives (`て…`, `けど…`, `たら…`) or hanging case particles (`彼が…`, `本当は…`).
  - **Case Marker Relational Bonus**: Rewards sentences with explicit case particles (`を`, `に`, `が`, `で`) that establish clear grammatical context for the target word.
  - **Length Sweet-Spot**: Distributes scores centered around 14–32 characters, eliminating both 4-character grunts and overwhelming run-on subtitles.
  - **Live Star Ratings**: Displays visual ratings (`★★★★★` to `★☆☆☆☆`) on the Mining Rank row in the TUI card preview.

- **⚡ Sub-Millisecond Morphological Analysis**:
  - Powered by the official WorksApplications [`sudachi.rs`](https://github.com/WorksApplications/sudachi.rs) engine.
  - Built-in grammar mergers for colloquial speech (`ねえ` $\to$ `ない`), small-tsu contractions (`っ`), and causative-passive verb inflections (`させられる`, `ちゃった`, `てしまう`).

- **📦 Ultra-Compact `.koto` Learning Bundles**:
  - Compresses heavy anime video files (1.4 GB+) into lightweight standalone learning archives (~14 MB, **>98% storage saved**).
  - Bundles subtitle tracks, 64kbps Opus audio slices, and 360p JPG thumbnails inside solid Tar + Zstandard archives.
  - **Hot-Swap Replacement**: Swap or align subtitles (`kotonoha bundle replace`) in milliseconds without re-encoding audio or re-taking screenshots.
  - **Duplicate Guard**: Automatically verifies subtitle fingerprints to protect against mis-pairing episodes.

- **🗺️ Sentence Explorer & Cherry-Picker (`--explore` / `-e`)**:
  - Interactive full-screen TUI powered by `ratatui`.
  - Classifies every line into difficulty tiers: $i+0$ (known), $i+1$ (target), $i+2$, $i+3+$.
  - Debounced auto-play audio on scroll (`↑`/`↓` / `k`/`j`), multi-card selection (`[✓]`), and instant offline dictionary lookup.

- **🎧 Headless Non-Blocking Audio Preview**:
  - Dedicated background playback daemon via `mpv` IPC sockets—zero terminal freezing or input stalling.
  - Press `Space` in the Subtitle Inspector (`--inspect`) to hear the line instantly.

- **📚 Offline Dictionary & Yomitan Integration**:
  - Dual-mode dictionary engine: Instant local SQLite queries (`JMdict_english` & `kanjium_pitch_accents`) with fallback to Jisho API.
  - Extracts clean sense definitions, parts of speech, and pitch accent classifications (`Heiban [0]`, `Atamadaka [1]`, `Nakadaka [n]`).

---

## 🚀 Quick Start

### 🪄 One-Liner Magic (Recommended)

Paste this into your terminal to install or update `kotonoha` automatically:

```bash
curl -sSL https://raw.githubusercontent.com/Praveensenpai/kotonoha/main/install.sh | bash
```

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
Run `kotonoha` without arguments to launch the interactive directory scanner:
```bash
kotonoha
```

### 2. Direct File Mining
Pass an anime video or subtitle file directly:
```bash
kotonoha "Frieren - 01.mkv"
```

### 3. Subtitle Inspector (`--inspect` / `-i`)
Review subtitle timestamps and hear selected dialogue on demand:
```bash
kotonoha --inspect "Frieren - 01.ja.srt"
```
- `↑` / `↓` : Navigate subtitle cards
- `Space` : Play/replay selected audio segment
- `Type` : Filter text dynamically; `Backspace` clears filter

### 4. Sentence Explorer & Cherry-Picker (`--explore` / `-e`)
Browse the entire episode by difficulty tier and cherry-pick cards for Anki:
```bash
kotonoha --explore "Frieren - 01.mkv"
```

| Key | Action |
| :--- | :--- |
| `↑` / `↓` or `k` / `j` | Navigate sentences (auto-plays audio snippet) |
| `←` / `→` or `Tab` | Switch target unknown word in multi-unknown sentences |
| `Space` or `x` | Toggle single card selection (`[✓]`) |
| `Shift` + `X` | Select all unknown words in current sentence |
| `Shift` + `C` | Clear all selections |
| `Enter` | Export selected cards into interactive Review / Mining session |
| `r` | Replay audio for active sentence |
| `a` | Toggle auto-play audio on scroll |
| `s` | Toggle sort: Difficulty ($i+0 \to i+3+$) ⇄ Chronological Timeline |
| `f` | Cycle difficulty filters: `All`, `[i+0]`, `[i+1] ★`, `[i+2]`, `[i+3+]` |
| `/` | Incremental search across Japanese text and vocabulary |
| `Esc` / `q` | Exit Explorer |

---

## 📦 .koto Bundles

Pre-save entire anime series into ultra-compact, portable `.koto` archives:

```bash
# 1. Create bundle from video + subtitle (1.4 GB MKV -> ~14 MB .koto)
kotonoha --bundle "Frieren - 01.mkv"

# 2. Mine or inspect anywhere without the original video file
kotonoha "Frieren - 01.koto"
kotonoha --explore "Frieren - 01.koto"

# 3. Hot-swap updated subtitles without re-encoding media
kotonoha bundle replace "Frieren - 01.koto" "Frieren - 01.improved.srt"

# 4. Open interactive bundle manager
kotonoha --bundles
```

---

## 🛠️ CLI Reference

| Command / Flag | Short | Description |
| :--- | :--- | :--- |
| `kotonoha` | | Launch interactive TUI file picker |
| `kotonoha <FILE>` | | Mine cards from video, subtitle, or `.koto` archive |
| `--bundle [FILE]` | `-b` | Pre-save video into ultra-compact `.koto` bundle |
| `--bundles` | `-B` | Interactive bundle manager (inspect, play, purge) |
| `bundle replace <KOTO> <SRT>` | | Hot-swap subtitle track inside existing bundle |
| `--clean-bundled` | `-C` | Purge raw source media after successful bundling |
| `--inspect [FILE]` | `-i` | Subtitle line inspector with `Space` audio playback |
| `--explore [FILE]` | `-e` | Full-screen sentence difficulty explorer & picker |
| `--config` | `-c` | Interactive configuration editor (storage, Anki, AI) |
| `--show-config` | `-S` | Display active configuration settings |
| `--manage-known` | `-k` | View and edit known vocabulary database |
| `--manage-mined` | `-m` | View and edit mined vocabulary cards |
| `--manage-ignored`| `-I` | View and edit ignored words list |
| `--sync` | `-s` | Push pending cards to Anki via AnkiConnect |
| `--completions [SHELL]` | | Output shell completions (`bash`, `zsh`, `fish`) |
| `--force` | `-f` | Force overwrite or re-encoding |
| `--version` | `-v` | Display version information |
| `--help` | `-h` | Display help screen |

---

## ⚙️ Configuration

`kotonoha` stores its configuration in `~/.config/kotonoha/config.toml`:

```toml
# Storage strategy for media bundles
bundle_dir = "~/.local/share/kotonoha/bundles"

# AnkiConnect integration
anki_deck = "Japanese::Immersion"
anki_model = "Japanese (Kotonoha)"
anki_url = "http://127.0.0.1:8765"

# Optional Gemini AI contextual parsing
gemini_api_key = "AIzaSy..."
```

---

## ⌨️ Shell Autocompletion

```bash
# Bash:
source <(kotonoha --completions bash)

# Zsh:
kotonoha --completions zsh > "${fpath[1]}/_kotonoha"

# Fish:
kotonoha --completions fish > ~/.config/fish/completions/kotonoha.fish
```

---

## 📜 License

Distributed under the [MIT License](LICENSE) © [Praveensenpai](https://github.com/Praveensenpai).
