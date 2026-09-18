# CODEBASE.md: kotonoha Semantic Digest

> **Notice**: This file is an AI-optimized semantic index adhering to the `codebase-digest` specification. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow

```text
┌───────────────────────────────┐
│ Input: Subtitle (.srt/.ass)   │
│ & Video (.mkv/.mp4/.koto)     │
└───────────────┬───────────────┘
                │
                ▼
┌───────────────────────────────┐
│ src/commands.rs & pairing.rs  │  ──> CLI Flag Dispatch / Media File Discovery
└───────────────┬───────────────┘
                │
                ▼
┌───────────────────────────────┐
│ srt.rs (Regex / Text Normal)  │  ──> Parses timestamps & clean dialogue
└───────────────┬───────────────┘
                │
                ▼
┌───────────────────────────────┐
│ nlp.rs (sudachi.rs + mergers) │  ──> Mode::C tokenization, POS filtering, grammar merging
└───────────────┬───────────────┘
                │
                ▼
┌───────────────────────────────┐
│ miner.rs (MiningEngine)       │  ──> Ranks exact i+1 sentences by freq & density tier
└───────────────┬───────────────┘
                │
                ▼
┌───────────────────────────────┐
│ session/ (Orchestration Loop) │ ◀──▶ db/ (SeaORM SQLite: known, ignored, cache, mined)
└───────┬───────────────┬───────┘
        │               │
        ▼               ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ dict/ (Local │ │ ai/ (Gemini  │ │ media.rs     │ │ anki/        │
│ JMdict+Pitch)│ │ Batch API)   │ │ ffmpeg / mpv │ │ AnkiConnect  │
└──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘
        │               │               │               │
        └───────────────┼───────────────┴───────────────┘
                        ▼
┌─────────────────────────────────────────────────────────────────┐
│ ui/ (Terminal TUI: Inquire, Ratatui, Card View, Subtitle Play)  │
└─────────────────────────────────────────────────────────────────┘
```

## 2. Global Constraints & Architecture Patterns

- **Primary Language & Edition**: Rust 2021 edition (`edition = "2021"`).
- **Architectural Paradigm**: Domain-driven, role-based architecture with clean separation of domain logic, infrastructure side-effects, session orchestration, and terminal UI presentation.
- **Hard Constraints**:
  - File size: <400 lines/file (soft 300).
  - Function length: <60 lines/function (soft 40).
  - Nesting depth: $\le 3$.
  - Parameters: $\le 4$ per function.
  - Zero tolerance: zero warnings under `-D warnings`, zero clippy warnings (`all = "deny"`), no `#[allow(dead_code)]`, zero production `unwrap()`/`expect()`.
- **Target Distribution**: Standalone Linux x86_64 binary installed to `~/.local/bin/kotonoha`, companion shell completions (`bash`, `zsh`, `fish`).
- **Data Directory Specs**: Follows XDG Base Directory specification:
  - Configuration: `~/.config/kotonoha/config.toml` (`dirs::config_dir()`)
  - Data / SQLite DB: `~/.local/share/kotonoha/kotonoha.db` (`dirs::data_dir()`)
  - Yomitan Dictionaries: `~/.local/share/kotonoha/dicts/`
  - Sudachi NLP Dictionary: `~/.local/share/kotonoha/sudachi/`
  - Audio/Image Media Cache: `~/.local/share/kotonoha/media/`
  - Bundles Directory: `~/.local/share/kotonoha/bundles/`
  - Bundle Runtime Cache: `~/.cache/kotonoha/bundles/` (`dirs::cache_dir()`)

---

## 3. Module & Interface Skeleton

### `src/main.rs` (Role: cli/entrypoint, Lines: 286)
- **Responsibility**: Application startup wiring, CLI dispatch, service status verification, auto-cleanup, and session initialization.
- **Imports**: `crate::{ai, anki, bundle, commands, config, db, dict, media, miner, nlp, session, srt, ui}`
- **Public Functions & Signatures**:
  ```rust
  #[tokio::main]
  async fn main() -> Result<()>
  ```
- **Consumers**: OS process execution.
- **Side Effects / I/O**: Reads CLI arguments, checks AnkiConnect endpoint, initializes SQLite DB, reads files, runs TUI.

### `src/commands.rs` (Role: cli/dispatcher, Lines: 348)
- **Responsibility**: CLI flag parsing, dispatching `--bundle`, `--bundles`, `--clean-bundled`, `--config`, `--show-config`, `--inspect`, `--manage-known`, `--manage-mined`, `--manage-ignored`, `--sync`, and completion scripts.
- **Imports**: `anyhow::Result`, `crate::{anki, bundle, config, db, nlp, srt, ui}`
- **Public Functions & Signatures**:
  ```rust
  pub async fn handle_cli_flag(arg: &str) -> Result<bool>
  ```
- **Consumers**: `src/main.rs`
- **Side Effects / I/O**: Terminal printing, runs interactive management subroutines, prints completion scripts.

### `src/commands/pairing.rs` (Role: cli/pairing, Lines: 238)
- **Responsibility**: Media file association, matching subtitle (`.srt`, `.ass`, `.vtt`) with video (`.mkv`, `.mp4`, `.webm`, `.koto`), handling stem normalization, bundle unpacking, and video-subtitle pairing lookups.
- **Imports**: `crate::{bundle, config, nlp::JapaneseTokenizer}`
- **Public Functions & Signatures**:
  ```rust
  pub fn words_with_readings(tokenizer: &JapaneseTokenizer, words: Vec<String>) -> Vec<(String, String)>
  pub fn find_paired_subtitle_for_video(vid_path: &Path) -> Option<PathBuf>
  pub fn find_paired_media(input_path: &Path) -> Result<(PathBuf, PathBuf)>
  pub fn find_paired_media_for_bundling(input_path: &Path) -> Result<(PathBuf, PathBuf)>
  ```
- **Consumers**: `src/main.rs`, `src/commands.rs`, `src/ui/picker.rs`
- **Side Effects / I/O**: Filesystem directory reads and lookups.

### `src/config.rs` (Role: domain/config, Lines: 284)
- **Responsibility**: App configuration persistence (`config.toml`), XDG path resolution, home tilde expansion, and legacy DB migration.
- **Imports**: `serde::{Deserialize, Serialize}`, `dirs`, `toml`
- **Types & Enums**:
  ```rust
  pub struct AnkiSettings { pub enable_sync: bool, pub connect_url: String, pub deck_name: String, pub model_name: String }
  pub struct AiSettings { pub enable_ai: bool, pub gemini_api_key: Option<String>, pub gemini_model: String, pub ai_batch_size: usize, pub ai_cache_ttl_minutes: usize }
  pub struct DictionarySettings { pub max_definition_senses: usize, pub max_glosses_per_sense: usize }
  pub enum BundleStorageStrategy { Colocated, Central, Subfolder }
  pub struct AppConfig { pub default_card_limit: usize, pub max_cached_cards: usize, pub media_dir: PathBuf, pub db_path: PathBuf, pub bundle_storage: BundleStorageStrategy, pub bundles_dir: PathBuf, pub audio_padding_secs: f64, pub anki: AnkiSettings, pub ai: AiSettings, pub dict: DictionarySettings }
  ```
- **Public Functions & Signatures**:
  ```rust
  pub fn default_data_dir() -> PathBuf
  pub fn default_config_dir() -> PathBuf
  impl AppConfig { pub fn load() -> Result<Self>; pub fn save(&self) -> Result<()>; }
  impl AiSettings { pub fn has_valid_api_key(&self) -> bool; }
  ```
- **Consumers**: `main.rs`, `commands.rs`, `session.rs`, `ui/config_menu.rs`
- **Side Effects / I/O**: Creates `~/.config/kotonoha/` and `~/.local/share/kotonoha/`, reads/writes `config.toml`, migrates legacy DB files.

### `src/srt.rs` (Role: domain/parser, Lines: 134)
- **Responsibility**: Subtitle parsing for SubRip (`.srt`) and Advanced SubStation Alpha (`.ass`/`.ssa`), HTML tag stripping, curly bracket stripping, and timestamp millisecond conversion.
- **Imports**: `regex::Regex`, `std::sync::LazyLock`
- **Types & Enums**:
  ```rust
  pub struct SubtitleSentence { pub index: usize, pub start_ms: u64, pub end_ms: u64, pub text: String, pub video_path: Option<PathBuf> }
  ```
- **Public Functions & Signatures**:
  ```rust
  pub fn parse_subtitle(path: &Path) -> Result<Vec<SubtitleSentence>>
  ```
- **Consumers**: `main.rs`, `commands.rs`, `session.rs`
- **Side Effects / I/O**: Reads subtitle files from disk.

### `src/nlp.rs` (Role: domain/nlp, Lines: 300)
- **Responsibility**: Japanese morphological tokenization via `sudachi.rs` (Mode C), kana conversions, POS classification (restricting `is_proper_noun` strictly to explicit proper/person/place name POS tags), audio grunt filtering, and coordinating grammar merger pipelines.
- **Imports**: `sudachi::analysis::{stateless_tokenizer::StatelessTokenizer, Mode, Tokenize}`, `sudachi::dic::dictionary::JapaneseDictionary`
- **Types & Enums**:
  ```rust
  pub struct TokenInfo { pub surface: String, pub dictionary_form: String, pub reading: String, pub is_content_word: bool, pub is_proper_noun: bool }
  pub struct SpannedToken { pub token: TokenInfo, pub begin: usize, pub end: usize }
  pub struct JapaneseTokenizer { dict: JapaneseDictionary }
  ```
- **Public Functions & Signatures**:
  ```rust
  pub fn kata_to_hira(s: &str) -> String
  impl JapaneseTokenizer { pub fn new() -> Result<Self>; pub fn tokenize(&self, text: &str) -> Result<Vec<TokenInfo>>; }
  ```
- **Consumers**: `miner.rs`, `commands.rs`, `session.rs`, `anki/formatter.rs`
- **Side Effects / I/O**: Ensures Sudachi dictionary exists, writes default `char.def`, `rewrite.def`, `unk.def`.

### `src/nlp/dictionary.rs` (Role: infra/nlp-dict, Lines: 60)
- **Responsibility**: Pure Rust automatic download and extraction of `sudachi-dictionary-latest-core.zip` (~72 MB) from CloudFront CDN if missing.
- **Imports**: `reqwest::blocking`, `zip::ZipArchive`
- **Public Functions & Signatures**:
  ```rust
  pub fn ensure_system_dict(dict_path: &Path) -> Result<()>
  ```
- **Consumers**: `src/nlp.rs`
- **Side Effects / I/O**: HTTP GET from CloudFront, unpacks `.dic` to disk.

### `src/nlp/mergers/` (Role: domain/nlp-mergers, Lines: ~520)
- **Files**: `mergers.rs`, `colloquial.rs`, `grammar.rs`, `verbs.rs`
- **Responsibility**: Normalizes and merges complex spoken Japanese tokens:
  - `colloquial.rs`: Negative verb endings (じゃない, ねえ), greetings (おはよう, こんにちは), small tsu drops, sentence-ending particles.
  - `grammar.rs`: Compound grammatical patterns (よりにもよって, もしかして, わけにはいかない, にあたって, について).
  - `verbs.rs`: Causative-passive inflections (させられる, ちゃった, てしまう), auxiliary stems, potential forms.
- **Consumers**: `src/nlp.rs`

### `src/miner.rs` (Role: domain/miner, Lines: 349)
- **Responsibility**: Core $i+1$ candidate discovery algorithm. Filters sentences with exactly one unknown content word, respects user ignored words, scores and ranks candidates using multi-factor sentence naturalness/completeness (`QualityScorer`), frequency, and density tier.
- **Submodules**: `pub mod quality;` (`src/miner/quality.rs`)
- **Imports**: `crate::{nlp::{JapaneseTokenizer, TokenInfo}, srt::SubtitleSentence}`, `quality::QualityScorer`
- **Types & Enums**:
  ```rust
  pub struct CandidateSentence { pub sentence: SubtitleSentence, pub target_word: String, pub target_reading: String, pub known_context_words: Vec<String>, pub unknown_context_words: Vec<String>, pub ignored_context_words: Vec<String>, pub episode_freq: usize, pub density_tier: usize, pub quality_score: f32, pub video_path: PathBuf }
  pub struct MiningEngine { tokenizer: JapaneseTokenizer }
  pub struct BuildCandidateParams<'a> { pub sentence: &'a SubtitleSentence, pub target_word: &'a str, pub tokens: &'a [TokenInfo], pub known_words: &'a HashSet<String>, pub ignored_words: &'a HashSet<String> }
  ```
- **Public Functions & Signatures**:
  ```rust
  impl MiningEngine {
      pub fn new(tokenizer: JapaneseTokenizer) -> Self;
      pub fn find_candidates(&self, sentences: &[SubtitleSentence], known_words: &HashSet<String>, ignored_words: &HashSet<String>) -> Vec<CandidateSentence>;
      pub fn build_candidate(p: BuildCandidateParams<'_>) -> CandidateSentence;
  }
  ```
- **Consumers**: `src/session.rs`, `src/ui/explorer/state.rs`

### `src/miner/quality.rs` (Role: domain/quality, Lines: 276)
- **Responsibility**: Multi-factor Japanese sentence naturalness, completeness, and flashcard suitability evaluator (`QualityScorer`). Evaluates predicate terminations (polite/terminal forms vs. dangling connective/particle cut-offs), case marker relationships, length distribution curves (14-32 char sweet spot), and interjection/grunt penalties.
- **Types & Enums**:
  ```rust
  pub struct QualityScorer;
  ```
- **Public Functions & Signatures**:
  ```rust
  impl QualityScorer {
      pub fn score(text: &str, target_word: &str, tokens: &[TokenInfo]) -> f32;
  }
  ```
- **Consumers**: `src/miner.rs`

### `src/dict/` (Role: domain/dict, Lines: ~900)
- **Files**: `dict.rs`, `service.rs`, `offline.rs`, `pitch.rs`, `context.rs`
- **Responsibility**:
  - `dict.rs`: `LookupResult { expression, reading, definition, pitch_accent }`.
  - `service.rs`: Dual-mode lookup (Offline SQLite first, fallback to Jisho API); verb stem unwinding fallbacks; candidate sorting.
  - `offline.rs`: Downloads and indexes Yomitan `JMdict_english.zip` (~15 MB) and `kanjium_pitch_accents.zip` (~1 MB) into SQLite table `offline_terms` for sub-millisecond local queries.
  - `pitch.rs`: Pitch accent pattern classification (`Heiban [0]`, `Atamadaka [1]`, `Nakadaka [n]`).
  - `context.rs`: Context-aware definition formatting, sense parsing, and placeholder filtering.
- **Public Functions & Signatures**:
  ```rust
  impl DictionaryService {
      pub async fn lookup_with_limits(client: &reqwest::Client, word: &str, max_senses: usize, max_glosses: usize) -> Result<LookupResult>;
      pub async fn lookup_all_candidates_cached(client: &reqwest::Client, db: Option<&Database>, word: &str, limits: LookupLimits) -> Result<Vec<LookupResult>>;
      pub async fn ensure_offline_dictionaries_ready(client: &reqwest::Client, db: &mut Database) -> Result<()>;
  }
  ```
- **Consumers**: `main.rs`, `session/mining.rs`, `session/ai_batch.rs`

### `src/bundle/` (Role: domain/bundle, Lines: ~1700)
- **Files**: `bundle.rs`, `create.rs`, `unpack.rs`, `archive.rs`, `manage.rs`, `replace.rs`, `duplicate_guard.rs`, `screenshots.rs`, `fingerprint.rs`, `destination.rs`
- **Responsibility**: Lightweight pre-saved `.koto` archives (>98.5% space saved over video).
  - `create.rs`: Orchestrates 3-step pipeline: Opus 64kbps extraction, parallel sentence screenshot generation, and Zstandard Tar packaging.
  - `unpack.rs`: Dynamically decompresses `.koto` archives into `~/.cache/kotonoha/bundles/<hash>/` on demand.
  - `replace.rs`: Hot-swaps internal `subtitles.srt` within existing `.koto` archives and updates manifest metadata (`--replace-sub`).
  - `duplicate_guard.rs`: Warns and prompts confirmation when a subtitle fingerprint has already been bundled with another episode or video.
  - `archive.rs`: Tar + Zstandard encoder/decoder at compression level 3.
  - `screenshots.rs`: Batch screenshot generation with fast bilinear scaling (360p) and keyframe skipping.
  - `fingerprint.rs`: Fast partial-hashing of video and subtitle files to avoid duplicate bundle work.
  - `destination.rs`: Resolves destination paths according to `BundleStorageStrategy` (`Colocated`, `Subfolder`, `Central`).
  - `manage.rs`: Inspects, lists, and purges bundled packages and source video files.
- **Consumers**: `commands.rs`, `commands/pairing.rs`, `ui/bundles.rs`

### `src/media.rs` (Role: infra/media, Lines: 385)
- **Responsibility**: External process integration with `ffmpeg` and `mpv`/audio daemons.
- **Public Functions & Signatures**:
  ```rust
  impl MediaExtractor {
      pub fn media_source_stem(video_path: &Path) -> String;
      pub fn card_media_stem(target_word: &str, video_path: &Path, start_ms: u64, index: usize) -> String;
      pub fn extract_preview_audio(video_path: &Path, start_ms: u64, end_ms: u64, output_path: &Path) -> Result<()>;
      pub fn extract_screenshot_with_index(video_path: &Path, timestamp_ms: u64, sentence_index: Option<usize>, output_path: &Path) -> Result<()>;
      pub fn play_preview_audio(audio_path: &Path) -> Option<std::process::Child>;
      pub fn play_subtitle_segment(video_path: &Path, start_ms: u64, end_ms: u64) -> Option<std::process::Child>;
      pub fn clean_old_media(media_dir: &Path, max_cards: usize, protected_paths: &HashSet<PathBuf>) -> Result<usize>;
  }
  ```
- **Consumers**: `session/card_actions.rs`, `session/media_preload.rs`, `ui/inspector.rs`, `main.rs`
- **Side Effects / I/O**: Spawns `ffmpeg` subprocesses, spawns background audio players (`mpv`, `pw-play`, `paplay`, `ffplay`), deletes expired media.

### `src/db/` (Role: infra/db, Lines: ~850)
- **Files**: `db.rs`, `words.rs`, `cards.rs`, `cache.rs`, `bundles.rs`, `entities.rs`, `entities/*.rs`
- **Responsibility**: SeaORM SQLite connection management, automatic table creation, column migrations, and queries for:
  - `known_words`: Vocabulary marked as known or mined (prevents re-mining).
  - `ignored_words`: Character names, sound effects, or skipped tokens.
  - `mined_cards`: Full history of mined cards with timestamps, note IDs, audio/image paths.
  - `dictionary_cache` & `all_candidates_cache`: Cached Jisho/JMdict responses.
  - `ai_analysis_cache`: Cached Gemini disambiguation results with TTL expiry.
  - `offline_terms`: Indexed Yomitan bilingual dictionary entries and pitch accents.
  - `bundled_media`: Registry of created `.koto` bundles and file fingerprints.
- **Consumers**: `main.rs`, `commands.rs`, `session.rs`, `dict/service.rs`
- **Side Effects / I/O**: SQLite file read/write with WAL mode.

### `src/ai.rs` (Role: infra/ai, Lines: 182)
- **Responsibility**: Google Gemini REST API client. Sends structured batches of sentences, target words, and dictionary candidates to obtain context-specific definition suggestions, sense selections, and segmentation warnings.
- **Imports**: `serde_json`, `reqwest`
- **Types & Enums**:
  ```rust
  pub struct AiAnalysisResult { pub card_index: usize, pub recommended_candidate_index: Option<usize>, pub recommended_sense_index: Option<usize>, pub parsing_warning: Option<String>, pub custom_definition_suggestion: Option<String>, pub explanation: Option<String>, pub english_natural: Option<String>, pub english_literal: Option<String>, pub kannada_natural: Option<String>, pub kannada_literal: Option<String> }
  pub struct CardBatchInput<'a> { pub card_index: usize, pub sentence: &'a str, pub target_word: &'a str, pub target_reading: &'a str, pub candidates: &'a [LookupResult] }
  pub struct GeminiAiService;
  ```
- **Public Functions & Signatures**:
  ```rust
  impl GeminiAiService { pub async fn analyze_batch(client: &reqwest::Client, api_key: &str, model: &str, cards: &[CardBatchInput<'_>]) -> Result<Vec<AiAnalysisResult>>; }
  ```
- **Consumers**: `src/session/ai_batch.rs`
- **Side Effects / I/O**: HTTPS POST to `generativelanguage.googleapis.com` with exponential retry backoff.

### `src/anki/` (Role: infra/anki, Lines: ~460)
- **Files**: `anki.rs`, `client.rs`, `formatter.rs`
- **Responsibility**:
  - `client.rs`: AnkiConnect HTTP client (`http://127.0.0.1:8765`). Verifies connection, creates decks, checks note models (`Japanese sentences+`), uploads media files (`storeMediaFile` with base64 data), queries existing notes (`findNotes`), and inserts notes (`addNote`).
  - `formatter.rs`: Formats HTML definition blocks, pitch accent graphs/numbers, furigana bracket notation (`Kanji[Reading]`), and search queries.
- **Consumers**: `commands.rs`, `session/mining.rs`
- **Side Effects / I/O**: HTTP POST to AnkiConnect API.

### `src/session/` (Role: domain/session, Lines: ~1310)
- **Files**: `session.rs`, `mining.rs`, `card_actions.rs`, `media_preload.rs`, `ai_batch.rs`
- **Responsibility**:
  - `session.rs`: High-frequency vocabulary bootstrapping (identifying names vs general vocab), line comprehension statistics calculation (known vs $i+1$ vs hard lines), mode selection.
  - `mining.rs`: Main interactive loop. Splits candidates into batches, dispatches background AI batch queries and media preloading, renders cards, executes user decisions, and auto-syncs to Anki upon completion.
  - `card_actions.rs`: Keyboard action handlers for card review: `m` (mine), `k` (known), `i` (ignore), `s` (skip), `c` (choose candidate), `d` (choose sense), `r` (edit reading), `p` (replay audio), `q` (quit).
  - `media_preload.rs`: Asynchronously pre-extracts Opus audio snippets and JPG screenshots in the background for upcoming batch items.
  - `ai_batch.rs`: Prepares candidate lookups, queries `ai_analysis_cache`, and invokes `GeminiAiService::analyze_batch` for uncached cards.
- **Consumers**: `src/main.rs`

### `src/ui/` (Role: tui, Lines: ~3500)
- **Files**: `ui.rs`, `card.rs`, `inspector.rs`, `picker.rs`, `picker/selector.rs`, `picker/state.rs`, `picker/render.rs`, `prompts.rs`, `bundles.rs`, `config_menu.rs`, `bootstrap.rs`, `helpers.rs`, `explorer.rs`, `explorer/model.rs`, `explorer/state.rs`, `explorer/render.rs`, `explorer/inspector_pane.rs`, `explorer/actions.rs`, `explorer/tests.rs`
- **Responsibility**:
  - `ui.rs`: Public facade for terminal UI interactions, exposing `TerminalUi::run_explorer` and `SessionMode::Explore`.
  - `explorer/`: Interactive Sentence Explorer & Difficulty Browser (`kotonoha --explore` or `-e`):
    - `model.rs`: Categorizes sentences by unknown count into tiers: $i+0$ (fully known), $i+1$, $i+2$, $i+3+$. Provides difficulty ($i+0 \to i+n$) and chronological timeline sorting.
    - `state.rs`: Controller state, offline caching, and multi-card selection tracking (`selected_cards: HashSet<(usize, String)>`). Handles `toggle_selection`, `toggle_all_in_current_sentence`, and `build_selected_candidates`. Zero AI network calls during browsing.
    - `render.rs` & `inspector_pane.rs`: Dual-pane ratatui interface. Left pane displays sentences with tier badges, selection checkboxes (`[✓]`, `[~]`, `[ ]`), and highlighted unknowns. Right pane displays active unknown word JMdict definitions and review selection status.
    - `actions.rs`: Debounced audio snippet playback on scroll.
    - Two-stage flow: Pressing `Enter` with selected cards transitions seamlessly into standard Kotonoha Card Review (`mining::run_mining_loop`) for candidate refinement, sense switching, custom readings, and Gemini AI analysis.
  - `card.rs`: Unicode box-drawing visual card layout, displaying target word, reading, pitch badge, frequency, highlighted sentence, definitions, AI suggestions, and key shortcuts.
  - `inspector.rs`: Subtitle inspection screen with live audio playback (`Space`), $i+1$ indicator badges (★), and instant text filtering.
  - `picker.rs` & `picker/`: Interactive Ratatui multi-select file picker:
    - `picker.rs`: Auto-discovery of media and subtitle files across standard directories, deduplicating paired `.srt` files and classifying media items with `SubtitleStatus`.
    - `picker/selector.rs`: Alternate-screen event loop with RAII `TerminalGuard` cleanup.
    - `picker/state.rs`: `SubtitleStatus` (`HasSub`, `Bundle`, `NoSub`), `CategoryFilter` (`All`, `Videos`, `Bundles` cycled via `Ctrl+F` / `F2`), multi-token whitespace search query filtering (spaces allowed in queries like `"yuru camp"`), `Tab`/`Shift+Tab` item toggle, `Ctrl+A`/`Ctrl+D` batch selection, and smooth offset scrolling.
    - `picker/render.rs`: Filter bar with active Category pill and dynamic match/selection stats, subtitle status badges (`[✓ SUB]`, `📦 [BUNDLE]`, `[NO SUB]`), hierarchical short path display (`~` prefix, dim directory, bold filename), and intuitive keybindings footer.
  - `prompts.rs`: Interactive selection prompts for candidates, dictionary senses, readings, and session modes (including `Explore`).
  - `bundles.rs`: TUI manager for `.koto` bundles and original media source cleanup.
  - `config_menu.rs`: Interactive editor for AI keys, models, Anki decks, and storage strategies.
  - `bootstrap.rs`: Multi-select checklists for marking initial known words and ignored character names.
  - `helpers.rs`: Japanese text width truncation (`unicode-width`) and duration formatting.
- **Consumers**: `main.rs`, `commands.rs`, `session.rs`

---

## 4. Execution Lifecycle Trace

1. **Startup & Pre-flight (`main.rs`)**:
   - Parses first positional argument. If a CLI flag (`--bundle`, `--inspect`, `--config`, etc.) is passed, dispatches to `commands::handle_cli_flag` and exits immediately.
   - Prints visual banner. Loads configuration from `~/.config/kotonoha/config.toml` (auto-migrating legacy paths).
   - Opens SQLite database (`kotonoha.db`), executes schema migrations.
   - Checks AnkiConnect availability; counts unsynced cards. Runs LRU cache cleaner on `~/.local/share/kotonoha/media/` preserving unsynced files.
   - Ensures offline Yomitan dictionaries (`JMdict`, `Kanjium`) and Sudachi dictionary are downloaded and indexed.
2. **Input Selection & Pairing (`commands/pairing.rs`)**:
   - If file arguments are passed via CLI, pairs each subtitle/video. If no arguments are provided, prompts user via interactive file picker.
   - Resolves paired subtitle (`.srt`/`.ass`) and video/audio (`.mkv`/`.mp4`/`.koto`).
3. **Comprehension Analysis & Mode Selection (`session.rs`)**:
   - Parses subtitle lines into `SubtitleSentence` structs.
   - If first run or new words detected, presents bootstrap prompts to batch-classify character names and frequent known words.
   - Tokenizes dialogue lines with `sudachi.rs`, categorizes lines into Known, $i+1$ Candidates, and Hard ($i+2+$) lines.
   - Displays comprehension ratio and prompts user for session mode (`MineI1Candidates` vs `ReviewKnownLines`).
4. **Batch Processing Loop (`session/mining.rs`)**:
   - Chunks eligible candidates into batches (default 25 cards).
   - Fires background media preloading (`media_preload.rs`) for audio and screenshots.
   - Collects candidate lookups and dispatches Gemini AI batch analysis (`session/ai_batch.rs`).
5. **Interactive Card Review (`session/card_actions.rs`, `ui/card.rs`)**:
   - Renders interactive card with target word, pitch accent, dictionary definitions, and highlighted sentence.
   - Plays audio snippet via background `mpv` / system player.
   - Awaits keyboard command:
     - `Enter` / `m`: Mine card to SQLite (`mined_cards`) with audio and image paths.
     - `k`: Mark word as known (`known_words`).
     - `i`: Add word to ignore list (`ignored_words`).
     - `c` / `d`: Switch dictionary candidate or specific sense.
     - `r`: Edit reading.
     - `p`: Replay audio preview.
     - `s`: Skip sentence.
     - `q`: Exit mining loop.
6. **Persistence & Sync (`anki/client.rs`)**:
   - After completing or exiting the mining session, checks if Anki is running.
   - If connected, automatically pushes mined cards to AnkiConnect: uploads audio/image assets and creates `Japanese sentences+` notes. Marks cards as synced in SQLite.
   - Prunes old cached media files according to `max_cached_cards`.

---

## 5. Verification Commands

```bash
# Build binary
cargo build --release

# Run full test suite
cargo test --all-targets

# Strict linting (warnings are denied)
cargo clippy --all-targets --all-features -- -D warnings

# Code format check
cargo fmt --check
```

---

## 6. Recent Iteration Changes

- **2026-09-14 (v0.0.70: Sentence Explorer Cherry-Picking & Direct Review Flow)**:
  - Overhauled Sentence Explorer into a snappy two-stage curation workflow: users browse sentences, toggle card selections (`[✓]` with `Space`/`x`, sentence with `X`, clear with `C`), and press `Enter` to directly enter the standard Kotonoha Card Review UI (`mining::run_mining_loop`).
  - Preserved full interactive card refinement controls: sense selection (`d`), custom reading (`r`), definition editing (`e`), Gemini AI contextual analysis (`g`), audio replay (`p`), and candidate switching (`c`) in the standard review stage.
  - Completely eliminated AI calls from Explorer browsing; word inspection uses fast, 100% offline JMdict lookups, preventing terminal buffer corruption, rate limits, and scrolling lag.
  - Added `BuildCandidateParams` and `MiningEngine::build_candidate` in `src/miner.rs` to reconstruct full `CandidateSentence` items from selected Explorer sentences and unknown words.
  - Added unit tests `test_build_candidate_from_explorer_selection` and `test_multi_unknown_candidate_generation` in `src/ui/explorer/tests.rs` (67/67 tests passing).
- **2026-09-14 (v0.0.69: Terminal Sanitization & Status Bar Error Surfacing)**:
  - Eliminated raw `eprintln!` writes to stderr during Gemini API batch retries in `src/ai.rs`, preventing Ratatui alternate-screen buffer corruption and top header scroll-off during TUI sessions.
  - Added structured HTTP 429 rate limit detection in `src/ai.rs`.
  - Routed AI contextual analysis errors into the bottom status bar (`ctrl.set_status`) in `src/ui/explorer/state.rs` with clean user-facing diagnostics.
  - Bumped crate version to `0.0.69`.
- **2026-09-14 (Sentence Explorer & Multi-Card Generation)**:
  - Added interactive Sentence Explorer & Difficulty Browser (`kotonoha --explore [FILE]` or `-e`, also selectable from session mode prompt).
  - Implemented sentence categorization by unknown count into difficulty tiers ($i+0 \to i+n$): $i+0$ (fully known), $i+1$, $i+2$, $i+3+$.
  - Built debounced auto-play audio on scroll (`↑`/`↓` / `k`/`j`), toggleable with `a`, manual replay with `r`/`Space`.
  - Added unknown word selector (`←`/`→` / `Tab`/`Shift+Tab`) with live visual highlight on the sentence line and instant dictionary/AI analysis updates in the right pane.
  - Implemented multi-card mining for $i+2+$ sentences (`M` or `Shift+M`), generating individual Anki cards for each unknown word while sharing sentence audio and screenshot frames.
  - Updated `src/anki/client.rs` (`find_existing_anki_note`) to match both `SentKanji` and `VocabKanji`, preventing multi-card Anki collisions on the same sentence.
  - Added unit test suite in `src/ui/explorer/tests.rs` (65/65 unit tests passing).
  - Updated shell completions (`bash`, `zsh`, `fish`) with `--explore` and `-e`.
- **2026-09-14 (Proper Nouns & Codebase Digest)**:
  - Fixed proper noun bypass issue in `src/miner.rs`, `src/session.rs`, and `src/ui/card.rs`: stopped unconditionally bypassing proper nouns; only skip them if explicitly present in `ignored_words`.
  - Tightened `is_proper_noun` in `src/nlp.rs` to only tag tokens with explicit proper/person/place name POS tags, eliminating overbroad Katakana noun misclassification and allowing standard Katakana vocabulary to be mined.
  - Added unit tests in `src/nlp/tests.rs` and `src/miner.rs` covering proper noun classification, ignored names, and katakana loanword mining.
- **2026-09-18 (v0.0.73: Naturalness & i+1 Quality Scorer)**:
  - Added `src/miner/quality.rs` (`QualityScorer`): multi-factor Japanese sentence naturalness, grammatical completeness, and flashcard suitability evaluator.
  - Replaced naive shortest-character-count heuristic with `QualityScorer` composite evaluation: predicate/copula termination checks, dangling connective/particle gating, case marker relational analysis, length sweet-spot curve (14-32 chars), and interjection penalty.
  - Added `quality_score: f32` to `CandidateSentence` and `CardRenderParams`.
  - Added star rating display (`★★★★★`) to the interactive terminal card preview UI (`src/ui/card.rs`, `src/session/card_actions.rs`).
  - Added full test suite in `src/miner/quality/tests.rs` (83/83 tests passing), including real-world anime subtitle dataset verification on *Yuru Camp* Ep 01.
