# CODEBASE.md: kotonoha Semantic Digest

> **Notice**: AI-optimized semantic index. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow
```text
Entrypoint ──> CLI/Parser ──> Domain Logic ──> Infra/IO
```

## 2. Global Constraints & Architecture Patterns
- **Primary Language**: Rust 2021 edition
- **Architectural Paradigm**: Role-based (domain/, infra/, api/cli/, tui/)
- **Hard Constraints**: <400 lines/file, <60 lines/fn, zero production unwrap(), 0 warnings.
- **Target Distribution**: Linux x86_64 standalone binary

## 3. Module & Interface Skeleton

### `src/ai/tests.rs` (Role: general, Lines: 129)
- **Responsibility**: Core general logic in src/ai/tests.rs
- **Imports**: use super :: build_prompt 

### `src/ai.rs` (Role: general, Lines: 336)
- **Responsibility**: Core general logic in src/ai.rs
- **Imports**: use anyhow :: Result , use serde :: { Deserialize , Serialize } 
- **Types & Enums**:
  ```rust
  pub struct AiAnalysisResult
  pub struct BatchAiAnalysisResponse
  pub struct CardBatchInput
  pub struct GeminiAiService
  pub struct UnifiedAiService
  pub struct DeepSeekAiService
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn analyze_batch (client : & reqwest :: Client , cfg : & crate :: config :: AppConfig , cards : & [CardBatchInput < '_ >] ,) -> Result < Vec < AiAnalysisResult > >
  async fn analyze_batch (client : & reqwest :: Client , url : & str , model : & str , api_key : & str , cards : & [CardBatchInput < '_ >] ,) -> Result < Vec < AiAnalysisResult > >
  async fn analyze_batch (client : & reqwest :: Client , api_key : & str , model : & str , cards : & [CardBatchInput < '_ >] ,) -> Result < Vec < AiAnalysisResult > >
  ```

### `src/anki/client.rs` (Role: cli, Lines: 330)
- **Responsibility**: Core cli logic in src/anki/client.rs
- **Imports**: use anyhow :: Result , use base64 :: engine :: general_purpose :: STANDARD as BASE64 , use base64 :: Engine , use std :: path :: Path , use crate :: config :: AppConfig , use crate :: db :: Database , use crate :: nlp :: JapaneseTokenizer , use super :: formatter :: { anki_search_text , format_definition_for_anki , pitch_pattern , sentence_with_furigana , } 
- **Public Functions & Signatures**:
  ```rust
  async fn anki_connected (url : & str) -> bool
  async fn anki_request (client : & reqwest :: Client , url : & str , action : & str , params : serde_json :: Value ,) -> Result < serde_json :: Value >
  async fn upload_anki_media (client : & reqwest :: Client , url : & str , card_id : i64 , path : & str , extension : & str ,) -> Result < Option < String > >
  async fn find_existing_anki_note (client : & reqwest :: Client , url : & str , model_name : & str , sentence : & str , target_word : Option < & str > ,) -> Result < Option < i64 > >
  async fn sync_to_anki (cfg : & AppConfig , db : & Database) -> Result < () >
  ```

### `src/anki/formatter.rs` (Role: general, Lines: 174)
- **Responsibility**: Core general logic in src/anki/formatter.rs
- **Imports**: use crate :: dict , use crate :: nlp :: JapaneseTokenizer 
- **Public Functions & Signatures**:
  ```rust
  fn escape_html (text : & str) -> String
  fn anki_search_text (text : & str) -> String
  fn format_definition_for_anki (def : & str) -> String
  fn sentence_with_furigana (tokenizer : & JapaneseTokenizer , sentence : & str , target_word : & str ,) -> String
  fn to_katakana (text : & str) -> String
  fn pitch_number (pitch_accent : & str , mora_count : usize) -> usize
  fn pitch_pattern (reading : & str , pitch_accent : & str) -> (String , String)
  ```

### `src/anki/tests.rs` (Role: general, Lines: 65)
- **Responsibility**: Core general logic in src/anki/tests.rs
- **Imports**: use super :: client :: * , use super :: formatter :: * 

### `src/anki.rs` (Role: general, Lines: 7)
- **Responsibility**: Core general logic in src/anki.rs
- **Imports**: pub use client :: * 

### `src/bundle/archive.rs` (Role: general, Lines: 150)
- **Responsibility**: Core general logic in src/bundle/archive.rs
- **Imports**: use anyhow :: { Context , Result } , use console :: style , use indicatif :: { ProgressBar , ProgressStyle } , use std :: fs :: File , use std :: path :: { Path , PathBuf } , use std :: time :: { Duration , Instant } , use tar :: Builder , use walkdir :: WalkDir , use zstd :: Encoder , use super :: unpack :: read_bundle_manifest , use super :: CreateBundleOptions , use crate :: ui :: format_duration 
- **Types & Enums**:
  ```rust
  pub struct BundleDurations
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn find_matching_bundle (final_output : & Path , video_fp : & str , sub_fp : & str , options : & CreateBundleOptions < '_ > ,) -> Option < PathBuf >
  fn package_bundle_archive (temp_dir : & Path , final_output : & Path) -> Result < Duration >
  fn print_bundle_summary (final_output : & Path , video_path : & Path , durations : BundleDurations)
  ```

### `src/bundle/create.rs` (Role: general, Lines: 301)
- **Responsibility**: Core general logic in src/bundle/create.rs
- **Imports**: use anyhow :: { Context , Result } , use console :: style , use indicatif :: { ProgressBar , ProgressStyle } , use std :: path :: { Path , PathBuf } , use std :: process :: { Command , Stdio } , use std :: time :: { Duration , Instant } , use crate :: srt :: { parse_subtitle , SubtitleSentence } , use crate :: ui :: format_duration , use super :: archive :: { find_matching_bundle , package_bundle_archive , print_bundle_summary , BundleDurations , } , use super :: destination :: resolve_bundle_destination , use super :: fingerprint :: { compute_subtitle_fingerprint , compute_video_fingerprint } , use super :: { BundleManifest , CreateBundleOptions } 
- **Public Functions & Signatures**:
  ```rust
  async fn create_bundle (video_path : & Path , subtitle_path : & Path , options : CreateBundleOptions < '_ > ,) -> Result < PathBuf >
  ```

### `src/bundle/destination.rs` (Role: general, Lines: 177)
- **Responsibility**: Core general logic in src/bundle/destination.rs
- **Imports**: use anyhow :: { Context , Result } , use console :: style , use std :: path :: { Path , PathBuf } , use crate :: config :: BundleStorageStrategy 
- **Public Functions & Signatures**:
  ```rust
  fn is_directory_writable (dir : & Path) -> bool
  fn resolve_bundle_destination (source_path : & Path , clean_stem : & str , strategy : BundleStorageStrategy , central_dir : & Path ,) -> Result < PathBuf >
  ```

### `src/bundle/duplicate_guard.rs` (Role: general, Lines: 135)
- **Responsibility**: Core general logic in src/bundle/duplicate_guard.rs
- **Imports**: use anyhow :: { Context , Result } , use console :: style , use inquire :: Confirm , use std :: path :: PathBuf , use super :: unpack :: read_bundle_manifest , use crate :: db :: Database 
- **Types & Enums**:
  ```rust
  pub struct DuplicateSubtitleMatch
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn check_duplicate_subtitle (video_name : & str , video_fp : & str , sub_fp : & str , db : Option < & Database > , search_dirs : & [PathBuf] ,) -> Option < DuplicateSubtitleMatch >
  fn prompt_duplicate_subtitle_warning (dup : & DuplicateSubtitleMatch , current_video_name : & str , sub_fp : & str , force : bool ,) -> Result < bool >
  ```

### `src/bundle/fingerprint.rs` (Role: general, Lines: 65)
- **Responsibility**: Core general logic in src/bundle/fingerprint.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: fs :: File , use std :: io :: { Read , Seek , SeekFrom } , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn compute_subtitle_fingerprint (subtitle_path : & Path) -> Result < String >
  fn compute_video_fingerprint (video_path : & Path) -> Result < String >
  ```

### `src/bundle/manage.rs` (Role: general, Lines: 244)
- **Responsibility**: Core general logic in src/bundle/manage.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: path :: { Path , PathBuf } , use crate :: db :: Database 
- **Types & Enums**:
  ```rust
  pub struct BundledSourceCleanupItem
  ```
- **Public Functions & Signatures**:
  ```rust
  fn total_source_size (& self) -> u64
  fn display_name (& self) -> String
  fn format_size (bytes : u64) -> String
  async fn get_bundled_items_with_existing_sources (db : & Database ,) -> Result < Vec < BundledSourceCleanupItem > >
  fn delete_source_media_files (items : & [BundledSourceCleanupItem]) -> Result < u64 >
  async fn delete_bundle_archive (bundle_path : & Path , db : Option < & Database >) -> Result < () >
  ```

### `src/bundle/replace.rs` (Role: general, Lines: 189)
- **Responsibility**: Core general logic in src/bundle/replace.rs
- **Imports**: use anyhow :: { Context , Result } , use console :: style , use std :: fs :: File , use std :: path :: { Path , PathBuf } , use super :: archive :: package_bundle_archive , use super :: fingerprint :: compute_subtitle_fingerprint , use super :: { get_bundles_cache_dir , BundleManifest } , use crate :: srt :: parse_subtitle 
- **Public Functions & Signatures**:
  ```rust
  async fn replace_bundle_subtitle (bundle_path : & Path , new_sub_path : & Path) -> Result < () >
  ```

### `src/bundle/screenshots.rs` (Role: general, Lines: 258)
- **Responsibility**: Core general logic in src/bundle/screenshots.rs
- **Imports**: use anyhow :: { Context , Result } , use indicatif :: ProgressBar , use rayon :: prelude :: * , use std :: path :: Path , use std :: process :: { Command , Stdio } , use crate :: media :: MediaExtractor , use crate :: srt :: SubtitleSentence 
- **Public Functions & Signatures**:
  ```rust
  fn detect_video_fps (video_path : & Path) -> f64
  fn find_best_keyframe (target_frame : u64 , kf_frames : & [u64]) -> u64
  fn extract_bundle_screenshots (video_path : & Path , sentences : & [SubtitleSentence] , dest_dir : & Path , pb : & ProgressBar ,) -> Result < () >
  ```

### `src/bundle/tests.rs` (Role: general, Lines: 346)
- **Responsibility**: Core general logic in src/bundle/tests.rs
- **Imports**: use super :: fingerprint :: * , use super :: * , use std :: fs :: File , use std :: io :: Write , use zip :: write :: SimpleFileOptions , use zip :: ZipWriter 

### `src/bundle/unpack.rs` (Role: general, Lines: 155)
- **Responsibility**: Core general logic in src/bundle/unpack.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: fs :: File , use std :: io :: Read , use std :: path :: Path , use zip :: ZipArchive , use super :: { get_bundles_cache_dir , BundleManifest , UnpackedBundle } 
- **Public Functions & Signatures**:
  ```rust
  fn unpack_bundle (koto_path : & Path) -> Result < UnpackedBundle >
  fn read_bundle_manifest (koto_path : & Path) -> Result < BundleManifest >
  ```

### `src/bundle.rs` (Role: general, Lines: 76)
- **Responsibility**: Core general logic in src/bundle.rs
- **Imports**: pub use create :: create_bundle , pub use manage :: * , pub use replace :: replace_bundle_subtitle , pub use unpack :: { read_bundle_manifest , unpack_bundle } , use serde :: { Deserialize , Serialize } , use std :: path :: { Path , PathBuf } , use crate :: db :: Database 
- **Types & Enums**:
  ```rust
  pub struct CreateBundleOptions
  pub struct BundleManifest
  pub struct UnpackedBundle
  ```
- **Public Functions & Signatures**:
  ```rust
  fn get_bundles_cache_dir () -> PathBuf
  fn is_bundle_file (path : & Path) -> bool
  fn is_bundle_dir (path : & Path) -> bool
  ```

### `src/commands/pairing.rs` (Role: general, Lines: 238)
- **Responsibility**: Core general logic in src/commands/pairing.rs
- **Imports**: use anyhow :: Result , use std :: path :: { Path , PathBuf } , use crate :: nlp :: JapaneseTokenizer 
- **Public Functions & Signatures**:
  ```rust
  fn words_with_readings (tokenizer : & JapaneseTokenizer , words : Vec < String > ,) -> Vec < (String , String) >
  fn find_paired_subtitle_for_video (vid_path : & Path) -> Option < PathBuf >
  fn find_paired_media (input_path : & Path) -> Result < (PathBuf , PathBuf) >
  fn find_paired_media_for_bundling (input_path : & Path) -> Result < (PathBuf , PathBuf) >
  ```

### `src/commands/queue/tests.rs` (Role: general, Lines: 82)
- **Responsibility**: Core general logic in src/commands/queue/tests.rs
- **Imports**: use super :: * , use std :: collections :: HashSet , use std :: path :: PathBuf 

### `src/commands/queue.rs` (Role: general, Lines: 251)
- **Responsibility**: Core general logic in src/commands/queue.rs
- **Imports**: use anyhow :: Result , use console :: style , use std :: collections :: HashSet , use std :: path :: Path , use crate :: commands , use crate :: config :: AppConfig , use crate :: db :: Database , use crate :: miner :: { context , BuildCandidateParams , CandidateSentence , MiningEngine } , use crate :: nlp :: JapaneseTokenizer , use crate :: queue :: { self , QueueEntry } , use crate :: srt :: { self , SubtitleSentence } , use crate :: ui :: queue :: QueueNoSubChoice , use crate :: ui :: TerminalUi 
- **Public Functions & Signatures**:
  ```rust
  async fn run_queue_command (cfg : & AppConfig , db : & mut Database , http_client : reqwest :: Client ,) -> Result < () >
  ```

### `src/commands.rs` (Role: general, Lines: 349)
- **Responsibility**: Core general logic in src/commands.rs
- **Imports**: pub use pairing :: * , use anyhow :: Result , use console :: style , use std :: path :: PathBuf , use crate :: anki , use crate :: config :: AppConfig , use crate :: db :: Database , use crate :: nlp :: JapaneseTokenizer , use crate :: srt :: parse_subtitle , use crate :: ui :: TerminalUi 
- **Public Functions & Signatures**:
  ```rust
  async fn handle_cli_flag (arg : & str) -> Result < bool >
  ```

### `src/config.rs` (Role: general, Lines: 317)
- **Responsibility**: Core general logic in src/config.rs
- **Imports**: use anyhow :: Result , use serde :: { Deserialize , Serialize } , use std :: path :: PathBuf 
- **Types & Enums**:
  ```rust
  pub struct AnkiSettings
  pub struct AiSettings
  pub struct DictionarySettings
  pub enum BundleStorageStrategy
  pub struct AppConfig
  ```
- **Public Functions & Signatures**:
  ```rust
  fn has_valid_api_key (& self) -> bool
  fn has_valid_gemini_key (& self) -> bool
  fn default_data_dir () -> PathBuf
  fn default_config_dir () -> PathBuf
  fn load () -> Result < Self >
  fn save (& self) -> Result < () >
  ```

### `src/db/bundles.rs` (Role: general, Lines: 115)
- **Responsibility**: Core general logic in src/db/bundles.rs
- **Imports**: use anyhow :: { Context , Result } , use sea_orm :: { sea_query :: OnConflict , ColumnTrait , EntityTrait , QueryFilter , QueryOrder , Set } , use std :: path :: { Path , PathBuf } , use super :: entities :: * , use super :: Database 
- **Public Functions & Signatures**:
  ```rust
  async fn record_bundle (& self , bundle_path : & Path , source_video : & str , source_subtitle : & str , video_fingerprint : & str , subtitle_fingerprint : & str ,) -> Result < () >
  async fn find_existing_bundle (& self , video_fingerprint : & str , subtitle_fingerprint : & str ,) -> Result < Option < PathBuf > >
  async fn find_bundle_by_subtitle_fingerprint (& self , subtitle_fingerprint : & str ,) -> Result < Option < bundled_media :: Model > >
  async fn get_all_bundled_media (& self) -> Result < Vec < bundled_media :: Model > >
  async fn delete_bundled_media_by_id (& self , id : i32) -> Result < () >
  async fn delete_bundled_media_by_path (& self , bundle_path : & str) -> Result < () >
  async fn prune_missing_bundles (& self) -> Result < usize >
  ```

### `src/db/cache.rs` (Role: general, Lines: 315)
- **Responsibility**: Core general logic in src/db/cache.rs
- **Imports**: use anyhow :: Result , use sea_orm :: { sea_query :: OnConflict , ColumnTrait , Condition , ConnectionTrait , EntityTrait , PaginatorTrait , QueryFilter , QueryOrder , QuerySelect , Set , } , use super :: entities :: * , use super :: Database 
- **Types & Enums**:
  ```rust
  pub struct GetCachedAiParams
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn get_cached_definition (& self , expression : & str ,) -> Result < Option < (String , String , String) > >
  async fn cache_definition (& self , expression : & str , reading : & str , definition : & str , pitch : & str ,) -> Result < () >
  async fn get_cached_candidates (& self , expression : & str ,) -> Result < Option < Vec < crate :: dict :: LookupResult > > >
  async fn cache_candidates (& self , expression : & str , candidates : & [crate :: dict :: LookupResult] ,) -> Result < () >
  async fn get_cached_ai_analysis (& self , p : GetCachedAiParams < '_ > ,) -> Result < Option < crate :: ai :: AiAnalysisResult > >
  async fn cache_ai_analysis (& self , sentence : & str , target_word : & str , model : & str , res : & crate :: ai :: AiAnalysisResult ,) -> Result < () >
  async fn clean_expired_ai_cache (& self , ttl_minutes : usize) -> Result < usize >
  async fn is_offline_dict_indexed (& self) -> Result < bool >
  async fn insert_offline_terms_batch (& mut self , terms : & [(String , String , String , String , String , i64)] ,) -> Result < usize >
  async fn query_offline_terms (& self , word : & str , exact_only : bool ,) -> Result < Vec < crate :: dict :: LookupResult > >
  ```

### `src/db/cards.rs` (Role: general, Lines: 138)
- **Responsibility**: Core general logic in src/db/cards.rs
- **Imports**: use anyhow :: Result , use sea_orm :: { ActiveModelTrait , ColumnTrait , EntityTrait , QueryFilter , QueryOrder , Set } , use std :: collections :: HashSet , use std :: path :: PathBuf , use super :: entities :: * , use super :: Database 
- **Types & Enums**:
  ```rust
  pub struct MinedCard
  pub struct SaveMinedCardParams
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn save_mined_card (& self , params : SaveMinedCardParams < '_ >) -> Result < i64 >
  async fn get_all_mined_cards (& self) -> Result < Vec < (MinedCard , Option < i64 >) > >
  async fn get_unsynced_mined_cards (& self) -> Result < Vec < MinedCard > >
  async fn mark_mined_card_synced (& self , card_id : i64 , anki_note_id : i64) -> Result < () >
  async fn get_unsynced_media_paths (& self) -> Result < HashSet < PathBuf > >
  ```

### `src/db/entities/ai_analysis_cache.rs` (Role: general, Lines: 24)
- **Responsibility**: Core general logic in src/db/entities/ai_analysis_cache.rs
- **Imports**: use sea_orm :: entity :: prelude :: * 
- **Types & Enums**:
  ```rust
  pub struct Model
  pub enum Relation
  ```

### `src/db/entities/all_candidates_cache.rs` (Role: general, Lines: 15)
- **Responsibility**: Core general logic in src/db/entities/all_candidates_cache.rs
- **Imports**: use sea_orm :: entity :: prelude :: * 
- **Types & Enums**:
  ```rust
  pub struct Model
  pub enum Relation
  ```

### `src/db/entities/bundled_media.rs` (Role: general, Lines: 20)
- **Responsibility**: Core general logic in src/db/entities/bundled_media.rs
- **Imports**: use sea_orm :: entity :: prelude :: * 
- **Types & Enums**:
  ```rust
  pub struct Model
  pub enum Relation
  ```

### `src/db/entities/dictionary_cache.rs` (Role: general, Lines: 17)
- **Responsibility**: Core general logic in src/db/entities/dictionary_cache.rs
- **Imports**: use sea_orm :: entity :: prelude :: * 
- **Types & Enums**:
  ```rust
  pub struct Model
  pub enum Relation
  ```

### `src/db/entities/ignored_words.rs` (Role: general, Lines: 14)
- **Responsibility**: Core general logic in src/db/entities/ignored_words.rs
- **Imports**: use sea_orm :: entity :: prelude :: * 
- **Types & Enums**:
  ```rust
  pub struct Model
  pub enum Relation
  ```

### `src/db/entities/known_words.rs` (Role: general, Lines: 15)
- **Responsibility**: Core general logic in src/db/entities/known_words.rs
- **Imports**: use sea_orm :: entity :: prelude :: * 
- **Types & Enums**:
  ```rust
  pub struct Model
  pub enum Relation
  ```

### `src/db/entities/mined_cards.rs` (Role: general, Lines: 26)
- **Responsibility**: Core general logic in src/db/entities/mined_cards.rs
- **Imports**: use sea_orm :: entity :: prelude :: * 
- **Types & Enums**:
  ```rust
  pub struct Model
  pub enum Relation
  ```

### `src/db/entities/offline_terms.rs` (Role: general, Lines: 19)
- **Responsibility**: Core general logic in src/db/entities/offline_terms.rs
- **Imports**: use sea_orm :: entity :: prelude :: * 
- **Types & Enums**:
  ```rust
  pub struct Model
  pub enum Relation
  ```

### `src/db/entities.rs` (Role: general, Lines: 17)
- **Responsibility**: Core general logic in src/db/entities.rs
- **Imports**: pub use ai_analysis_cache :: Entity as AiAnalysisCache , pub use all_candidates_cache :: Entity as AllCandidatesCache , pub use bundled_media :: Entity as BundledMedia , pub use dictionary_cache :: Entity as DictionaryCache , pub use ignored_words :: Entity as IgnoredWords , pub use known_words :: Entity as KnownWords , pub use mined_cards :: Entity as MinedCards , pub use offline_terms :: Entity as OfflineTerms 

### `src/db/words.rs` (Role: general, Lines: 130)
- **Responsibility**: Core general logic in src/db/words.rs
- **Imports**: use anyhow :: Result , use sea_orm :: { sea_query :: OnConflict , ColumnTrait , EntityTrait , QueryFilter , QueryOrder , Set } , use std :: collections :: HashSet , use super :: entities :: * , use super :: Database 
- **Public Functions & Signatures**:
  ```rust
  async fn get_known_words (& self) -> Result < HashSet < String > >
  async fn get_known_words_by_source (& self , source : & str) -> Result < HashSet < String > >
  async fn add_known_words (& self , words : & [String]) -> Result < usize >
  async fn add_known_words_with_source (& self , words : & [String] , source : & str ,) -> Result < usize >
  async fn get_known_words_sorted_by_source (& self , source : & str) -> Result < Vec < String > >
  async fn remove_known_words (& self , words : & [String]) -> Result < usize >
  async fn get_ignored_words (& self) -> Result < HashSet < String > >
  async fn add_ignored_word (& self , word : & str) -> Result < () >
  async fn get_ignored_words_sorted (& self) -> Result < Vec < String > >
  async fn remove_ignored_words (& self , words : & [String]) -> Result < usize >
  ```

### `src/db.rs` (Role: general, Lines: 152)
- **Responsibility**: Core general logic in src/db.rs
- **Imports**: use anyhow :: { Context , Result } , use sea_orm :: { ConnectionTrait , Database as SeaDatabase , DatabaseConnection } , use std :: path :: Path , pub use cache :: * , pub use cards :: * , pub use entities :: * 
- **Types & Enums**:
  ```rust
  pub struct Database
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn open (db_path : & Path) -> Result < Self >
  ```

### `src/dict/context.rs` (Role: general, Lines: 257)
- **Responsibility**: Core general logic in src/dict/context.rs
- **Types & Enums**:
  ```rust
  pub enum ContextHint
  ```
- **Public Functions & Signatures**:
  ```rust
  fn context_hint (sentence : & str , target_word : & str) -> Option < ContextHint >
  fn sense_line_score (line : & str , hint : ContextHint) -> i32
  fn format_contextual_definition (definition : & str , hint : Option < ContextHint > , max_senses : usize , max_glosses : usize ,) -> String
  fn has_contextual_sense (definition : & str , hint : ContextHint) -> bool
  fn parse_senses (def : & str) -> Vec < String >
  fn is_placeholder_definition (definition : & str) -> bool
  fn truncate_definition (def : & str , max_senses : usize , max_glosses : usize) -> String
  fn sort_candidates_for_context (candidates : & mut [crate :: dict :: LookupResult] , target_word : & str , target_reading : & str ,)
  ```

### `src/dict/offline.rs` (Role: general, Lines: 262)
- **Responsibility**: Core general logic in src/dict/offline.rs
- **Imports**: use anyhow :: Result , use indicatif :: { ProgressBar , ProgressStyle } , use std :: io :: Read 
- **Public Functions & Signatures**:
  ```rust
  async fn ensure_offline_dictionaries_ready (client : & reqwest :: Client , db : & mut crate :: db :: Database ,) -> Result < () >
  async fn download_file_with_progress (client : & reqwest :: Client , url : & str , target_path : & std :: path :: Path ,) -> Result < () >
  fn extract_text_from_yomitan_json (v : & serde_json :: Value , out : & mut Vec < String >)
  ```

### `src/dict/pitch.rs` (Role: general, Lines: 63)
- **Responsibility**: Core general logic in src/dict/pitch.rs
- **Public Functions & Signatures**:
  ```rust
  fn split_morae (reading : & str) -> Vec < String >
  fn format_pitch_accent (reading : & str , pitch_num : usize) -> (String , String , usize)
  ```

### `src/dict/service.rs` (Role: general, Lines: 363)
- **Responsibility**: Core general logic in src/dict/service.rs
- **Imports**: use anyhow :: Result , use super :: context :: is_placeholder_definition , use super :: offline , use super :: LookupResult 
- **Types & Enums**:
  ```rust
  pub struct LookupLimits
  pub struct DictionaryService
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn lookup (client : & reqwest :: Client , word : & str) -> Result < LookupResult >
  async fn lookup_with_limits (client : & reqwest :: Client , word : & str , max_senses : usize , max_glosses : usize ,) -> Result < LookupResult >
  fn parse_entry (data : & serde_json :: Value , word : & str , max_senses : usize , max_glosses : usize ,) -> LookupResult
  async fn lookup_all_candidates (client : & reqwest :: Client , word : & str , max_senses : usize , max_glosses : usize ,) -> Result < Vec < LookupResult > >
  async fn ensure_offline_dictionaries_ready (client : & reqwest :: Client , db : & mut crate :: db :: Database ,) -> Result < () >
  async fn lookup_all_candidates_cached (client : & reqwest :: Client , db : Option < & crate :: db :: Database > , word : & str , limits : LookupLimits ,) -> Result < Vec < LookupResult > >
  ```

### `src/dict/tests.rs` (Role: general, Lines: 300)
- **Responsibility**: Core general logic in src/dict/tests.rs
- **Imports**: use super :: * 

### `src/dict.rs` (Role: general, Lines: 21)
- **Responsibility**: Core general logic in src/dict.rs
- **Imports**: pub use context :: * , pub use pitch :: * , pub use service :: * , use serde :: { Deserialize , Serialize } 
- **Types & Enums**:
  ```rust
  pub struct LookupResult
  ```

### `src/main.rs` (Role: general, Lines: 302)
- **Responsibility**: Core general logic in src/main.rs
- **Imports**: use anyhow :: Result , use config :: AppConfig , use console :: style , use db :: Database , use dict :: DictionaryService , use srt :: parse_subtitle , use std :: path :: { Path , PathBuf } , use ui :: TerminalUi 

### `src/media/tests.rs` (Role: general, Lines: 121)
- **Responsibility**: Core general logic in src/media/tests.rs
- **Imports**: use super :: * , use std :: collections :: HashSet , use std :: fs :: File 

### `src/media/title.rs` (Role: general, Lines: 72)
- **Responsibility**: Core general logic in src/media/title.rs
- **Imports**: use regex :: Regex , use std :: path :: Path , use std :: sync :: LazyLock , use super :: MediaExtractor 
- **Public Functions & Signatures**:
  ```rust
  fn extract_clean_show_context (video_path : & Path) -> String
  ```

### `src/media.rs` (Role: general, Lines: 388)
- **Responsibility**: Core general logic in src/media.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: path :: Path , use std :: process :: { Command , Stdio } , pub use title :: extract_clean_show_context 
- **Types & Enums**:
  ```rust
  pub struct MediaExtractor
  ```
- **Public Functions & Signatures**:
  ```rust
  fn media_source_stem (video_path : & Path) -> String
  fn card_media_stem (target_word : & str , video_path : & Path , start_ms : u64 , index : usize ,) -> String
  fn extract_preview_audio (video_path : & Path , start_ms : u64 , end_ms : u64 , output_path : & Path ,) -> Result < () >
  fn extract_screenshot_with_index (video_path : & Path , timestamp_ms : u64 , sentence_index : Option < usize > , output_path : & Path ,) -> Result < () >
  fn extract_screenshot (video_path : & Path , timestamp_ms : u64 , output_path : & Path ,) -> Result < () >
  fn play_preview_audio (audio_path : & Path) -> Option < std :: process :: Child >
  fn play_subtitle_segment (video_path : & Path , start_ms : u64 , end_ms : u64 ,) -> Option < std :: process :: Child >
  fn clean_old_media (media_dir : & Path , max_cards : usize , protected_paths : & std :: collections :: HashSet < std :: path :: PathBuf > ,) -> Result < usize >
  ```

### `src/miner/context.rs` (Role: general, Lines: 83)
- **Responsibility**: Core general logic in src/miner/context.rs
- **Imports**: use crate :: srt :: SubtitleSentence 
- **Public Functions & Signatures**:
  ```rust
  fn extract_dialogue_context (sentences : & [SubtitleSentence] , target_idx : usize , max_lines_before : usize , max_lines_after : usize , max_gap_ms : u64 ,) -> (Vec < String > , Vec < String >)
  fn extract_dialogue_context_for_sentence (sentences : & [SubtitleSentence] , target : & SubtitleSentence , max_lines_before : usize , max_lines_after : usize , max_gap_ms : u64 ,) -> (Vec < String > , Vec < String >)
  ```

### `src/miner/quality/tests.rs` (Role: general, Lines: 326)
- **Responsibility**: Core general logic in src/miner/quality/tests.rs
- **Imports**: use super :: * , use crate :: nlp :: TokenInfo 

### `src/miner/quality.rs` (Role: general, Lines: 276)
- **Responsibility**: Core general logic in src/miner/quality.rs
- **Imports**: use crate :: nlp :: TokenInfo 
- **Types & Enums**:
  ```rust
  pub struct QualityScorer
  ```
- **Public Functions & Signatures**:
  ```rust
  fn score (text : & str , target_word : & str , tokens : & [TokenInfo]) -> f32
  ```

### `src/miner/tests.rs` (Role: general, Lines: 236)
- **Responsibility**: Core general logic in src/miner/tests.rs
- **Imports**: use super :: * , use crate :: srt :: SubtitleSentence 

### `src/miner.rs` (Role: general, Lines: 270)
- **Responsibility**: Core general logic in src/miner.rs
- **Imports**: use self :: context :: { extract_dialogue_context , DEFAULT_MAX_CONTEXT_LINES , DEFAULT_MAX_DIALOGUE_GAP_MS , } , use self :: quality :: QualityScorer , use crate :: nlp :: JapaneseTokenizer , use crate :: srt :: SubtitleSentence , use std :: collections :: HashSet 
- **Types & Enums**:
  ```rust
  pub struct CandidateSentence
  pub struct MiningEngine
  pub struct BuildCandidateParams
  ```
- **Public Functions & Signatures**:
  ```rust
  fn new (tokenizer : JapaneseTokenizer) -> Self
  fn find_candidates (& self , sentences : & [SubtitleSentence] , known_words : & HashSet < String > , ignored_words : & HashSet < String > ,) -> Vec < CandidateSentence >
  fn build_candidate (p : BuildCandidateParams < '_ >) -> CandidateSentence
  ```

### `src/nlp/dictionary.rs` (Role: general, Lines: 59)
- **Responsibility**: Core general logic in src/nlp/dictionary.rs
- **Imports**: use anyhow :: { bail , Context , Result } , use std :: fs :: File , use std :: io :: Cursor , use std :: path :: Path 
- **Public Functions & Signatures**:
  ```rust
  fn ensure_system_dict (dict_path : & Path) -> Result < () >
  ```

### `src/nlp/filters.rs` (Role: general, Lines: 260)
- **Responsibility**: Core general logic in src/nlp/filters.rs
- **Types & Enums**:
  ```rust
  pub struct MorphemeMeta
  ```
- **Public Functions & Signatures**:
  ```rust
  fn is_formal_noun (dict_form : & str) -> bool
  fn is_conjunction_particle (dict_form : & str) -> bool
  fn is_audio_grunt (dict_form : & str , surface : & str) -> bool
  fn is_scream_or_noise (dict_form : & str , surface : & str) -> bool
  fn is_symbol_or_junk (meta : & MorphemeMeta < '_ >) -> bool
  fn is_predicate_suffix (is_content_word : bool , surface : & str) -> bool
  fn is_predicate_lemma (dict_form : & str) -> bool
  fn normalize_canonical_lemma_reading (dictionary_form : & str , current_reading : & str) -> String
  fn is_name_honorific (surface : & str) -> bool
  ```

### `src/nlp/mergers/colloquial.rs` (Role: tui, Lines: 220)
- **Responsibility**: Core tui logic in src/nlp/mergers/colloquial.rs
- **Imports**: use crate :: nlp :: { kata_to_hira , SpannedToken , TokenInfo } 
- **Public Functions & Signatures**:
  ```rust
  fn normalize_colloquial_negative (surface : & str , dictionary_form : String , reading : String ,) -> (String , String)
  fn merge_colloquial_small_tsu (tokens : Vec < SpannedToken >) -> Vec < TokenInfo >
  fn normalize_explanatory_nan (tokens : & mut [SpannedToken] , text : & str)
  fn normalize_colloquial_greetings (tokens : & mut Vec < SpannedToken >)
  fn normalize_kansai_negative (tokens : & mut Vec < SpannedToken >)
  fn normalize_explanatory_njanai (tokens : & mut [SpannedToken])
  ```

### `src/nlp/mergers/counters/numerals.rs` (Role: general, Lines: 228)
- **Responsibility**: Core general logic in src/nlp/mergers/counters/numerals.rs
- **Public Functions & Signatures**:
  ```rust
  fn is_numeric_token (surface : & str) -> bool
  fn digits_to_kanji (s : & str) -> String
  fn is_lexical_counter_compound (kanji_num : & str , counter : & str) -> bool
  fn derive_special_reading (kanji_num : & str , counter : & str) -> Option < & 'static str >
  fn derive_numeric_reading (surface : & str , raw_reading : & str) -> String
  ```

### `src/nlp/mergers/counters/tests.rs` (Role: general, Lines: 139)
- **Responsibility**: Core general logic in src/nlp/mergers/counters/tests.rs
- **Imports**: use crate :: nlp :: JapaneseTokenizer 

### `src/nlp/mergers/counters.rs` (Role: general, Lines: 204)
- **Responsibility**: Core general logic in src/nlp/mergers/counters.rs
- **Imports**: use crate :: nlp :: { SpannedToken , TokenInfo } , use numerals :: * 
- **Public Functions & Signatures**:
  ```rust
  fn merge_number_counters (tokens : Vec < SpannedToken >) -> Vec < SpannedToken >
  ```

### `src/nlp/mergers/grammar.rs` (Role: general, Lines: 225)
- **Responsibility**: Core general logic in src/nlp/mergers/grammar.rs
- **Imports**: use crate :: nlp :: { kata_to_hira , SpannedToken , TokenInfo } 
- **Public Functions & Signatures**:
  ```rust
  fn merge_adverb_naru (tokens : Vec < SpannedToken >) -> Vec < SpannedToken >
  fn merge_fixed_expression (tokens : Vec < SpannedToken > , expression : & str) -> Vec < SpannedToken >
  fn merge_grammar_expressions (mut tokens : Vec < SpannedToken >) -> Vec < SpannedToken >
  ```

### `src/nlp/mergers/verbs.rs` (Role: general, Lines: 262)
- **Responsibility**: Core general logic in src/nlp/mergers/verbs.rs
- **Imports**: use crate :: nlp :: { SpannedToken , TokenInfo } 
- **Public Functions & Signatures**:
  ```rust
  fn merge_complex_verb_inflections (tokens : Vec < SpannedToken >) -> Vec < SpannedToken >
  fn merge_compound_verbs (tokens : Vec < SpannedToken >) -> Vec < SpannedToken >
  fn normalize_subsidiary_verb_lemma (surface : & str , raw_dict : & str , pos_type : & str , _pos_form : & str ,) -> String
  fn normalize_ambiguous_imperatives (tokens : & mut [SpannedToken] , text : & str)
  ```

### `src/nlp/mergers.rs` (Role: general, Lines: 9)
- **Responsibility**: Core general logic in src/nlp/mergers.rs
- **Imports**: pub use colloquial :: * , pub use counters :: * , pub use grammar :: * , pub use verbs :: * 

### `src/nlp/tests.rs` (Role: general, Lines: 389)
- **Responsibility**: Core general logic in src/nlp/tests.rs
- **Imports**: use super :: mergers :: normalize_colloquial_negative 

### `src/nlp.rs` (Role: general, Lines: 319)
- **Responsibility**: Core general logic in src/nlp.rs
- **Imports**: pub use filters :: { is_predicate_lemma , is_predicate_suffix } , use anyhow :: Result , use filters :: { is_conjunction_particle , is_formal_noun , is_symbol_or_junk , MorphemeMeta } , use std :: path :: PathBuf , use sudachi :: analysis :: stateless_tokenizer :: StatelessTokenizer , use sudachi :: analysis :: Mode , use sudachi :: analysis :: Tokenize , use sudachi :: config :: Config , use sudachi :: dic :: dictionary :: JapaneseDictionary 
- **Types & Enums**:
  ```rust
  pub struct TokenInfo
  pub struct SpannedToken
  pub struct JapaneseTokenizer
  ```
- **Public Functions & Signatures**:
  ```rust
  fn kata_to_hira (s : & str) -> String
  fn new () -> Result < Self >
  fn tokenize (& self , text : & str) -> Result < Vec < TokenInfo > >
  ```

### `src/queue/tests.rs` (Role: general, Lines: 84)
- **Responsibility**: Core general logic in src/queue/tests.rs
- **Imports**: use super :: * , use std :: fs 

### `src/queue.rs` (Role: general, Lines: 101)
- **Responsibility**: Core general logic in src/queue.rs
- **Imports**: use anyhow :: Result , use serde :: { Deserialize , Serialize } , use std :: path :: { Path , PathBuf } 
- **Types & Enums**:
  ```rust
  pub struct QueueEntry
  ```
- **Public Functions & Signatures**:
  ```rust
  fn queue_file_path () -> PathBuf
  fn read_queue (path : & Path) -> Result < Vec < QueueEntry > >
  fn append_entry (path : & Path , entry : & QueueEntry) -> Result < () >
  fn ensure_mpv_companion_script_installed () -> Result < () >
  fn remove_entries (path : & Path , indices_to_remove : & [usize]) -> Result < () >
  ```

### `src/session/ai_batch.rs` (Role: general, Lines: 260)
- **Responsibility**: Core general logic in src/session/ai_batch.rs
- **Imports**: use anyhow :: Result , use std :: collections :: HashMap , use std :: sync :: Arc , use tokio :: task :: JoinHandle , use crate :: ai , use crate :: config :: AppConfig , use crate :: db :: Database , use crate :: dict :: { self , DictionaryService , LookupLimits } , use crate :: miner :: CandidateSentence 
- **Types & Enums**:
  ```rust
  pub struct AiBatchPreparation
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn prepare_ai_batch (candidates_to_process : & [CandidateSentence] , cfg : & AppConfig , db : & Database , http_client : & Arc < reqwest :: Client > ,) -> AiBatchPreparation
  async fn collect_ai_results (prep : AiBatchPreparation , candidates : & [CandidateSentence] , cfg : & AppConfig , db : & Database , ai_start : std :: time :: Instant ,) -> HashMap < usize , ai :: AiAnalysisResult >
  ```

### `src/session/card_actions.rs` (Role: general, Lines: 183)
- **Responsibility**: Core general logic in src/session/card_actions.rs
- **Imports**: use anyhow :: Result , use std :: path :: Path , use std :: sync :: Arc , use crate :: ai :: AiAnalysisResult , use crate :: config :: AppConfig , use crate :: db :: { Database , SaveMinedCardParams } , use crate :: dict :: { self , DictionaryService , LookupLimits , LookupResult } , use crate :: media :: MediaExtractor , use crate :: miner :: CandidateSentence , use crate :: ui :: card_dashboard :: { DashboardAction , DashboardParams } , use crate :: ui :: TerminalUi 
- **Types & Enums**:
  ```rust
  pub enum CardActionResult
  pub struct CardActionContext
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn handle_card_interaction (mut ctx : CardActionContext < '_ >) -> Result < CardActionResult >
  ```

### `src/session/media_preload.rs` (Role: general, Lines: 156)
- **Responsibility**: Core general logic in src/session/media_preload.rs
- **Imports**: use indicatif :: { ProgressBar , ProgressStyle } , use rayon :: prelude :: * , use std :: sync :: Arc , use crate :: config :: AppConfig , use crate :: db :: Database , use crate :: dict :: { self , DictionaryService } , use crate :: media :: MediaExtractor , use crate :: miner :: CandidateSentence 
- **Public Functions & Signatures**:
  ```rust
  async fn preload_batch_media (candidates_to_process : & [CandidateSentence] , cfg : & AppConfig , db : & mut Database , http_client : & Arc < reqwest :: Client > ,)
  ```

### `src/session/mining.rs` (Role: general, Lines: 335)
- **Responsibility**: Core general logic in src/session/mining.rs
- **Imports**: use anyhow :: Result , use console :: style , use std :: sync :: Arc , use super :: ai_batch :: { collect_ai_results , prepare_ai_batch } , use super :: card_actions :: { handle_card_interaction , CardActionContext , CardActionResult } , use super :: media_preload :: preload_batch_media , use crate :: ai , use crate :: anki , use crate :: config :: AppConfig , use crate :: db :: Database , use crate :: dict :: { self , DictionaryService , LookupLimits } , use crate :: miner :: CandidateSentence , use crate :: ui :: TerminalUi 
- **Public Functions & Signatures**:
  ```rust
  async fn run_mining_loop (all_candidates : Vec < CandidateSentence > , cfg : & AppConfig , db : & mut Database , http_client : reqwest :: Client ,) -> Result < () >
  ```

### `src/session.rs` (Role: general, Lines: 344)
- **Responsibility**: Core general logic in src/session.rs
- **Imports**: use anyhow :: Result , use console :: style , use std :: collections :: { HashMap , HashSet } , use crate :: config :: AppConfig , use crate :: db :: Database , use crate :: miner :: { CandidateSentence , MiningEngine } , use crate :: nlp :: JapaneseTokenizer , use crate :: srt :: SubtitleSentence , use crate :: ui :: { SessionMode , TerminalUi } 
- **Types & Enums**:
  ```rust
  pub struct ComprehensionStats
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn bootstrap_vocabulary (sentences : & [SubtitleSentence] , tokenizer : & JapaneseTokenizer , db : & mut Database , known_words : & HashSet < String > , ignored_words : & HashSet < String > ,) -> Result < () >
  async fn calculate_comprehension_stats (sentences : & [SubtitleSentence] , tokenizer : & JapaneseTokenizer , db : & Database , ignored_words : & HashSet < String > ,) -> Result < ComprehensionStats >
  fn collect_review_known_candidates (sentences : & [SubtitleSentence] , tokenizer : & JapaneseTokenizer , known_words : & HashSet < String > , ignored_words : & HashSet < String > ,) -> Vec < CandidateSentence >
  async fn run_session (sentences : Vec < SubtitleSentence > , cfg : & AppConfig , mut db : Database , http_client : reqwest :: Client ,) -> Result < () >
  ```

### `src/srt.rs` (Role: general, Lines: 142)
- **Responsibility**: Core general logic in src/srt.rs
- **Imports**: use anyhow :: { Context , Result } , use regex :: Regex , use std :: path :: Path , use std :: sync :: LazyLock 
- **Types & Enums**:
  ```rust
  pub struct SubtitleSentence
  ```
- **Public Functions & Signatures**:
  ```rust
  fn parse_subtitle (path : & Path) -> Result < Vec < SubtitleSentence > >
  ```

### `src/ui/bootstrap.rs` (Role: tui, Lines: 189)
- **Responsibility**: Core tui logic in src/ui/bootstrap.rs
- **Imports**: use anyhow :: Result , use inquire :: MultiSelect , use super :: helpers :: format_word_with_reading 
- **Public Functions & Signatures**:
  ```rust
  fn bootstrap_known_words (vocab_items : & [(String , usize , String)]) -> Result < Vec < String > >
  fn bootstrap_ignored_names (vocab_items : & [(String , usize , String)]) -> Result < Vec < String > >
  fn manage_known_words (words : & [(String , String)]) -> Result < Vec < String > >
  fn manage_ignored_words (words : & [(String , String)]) -> Result < Vec < String > >
  fn manage_mined_words (words : & [(String , String)]) -> Result < Vec < String > >
  ```

### `src/ui/bundles.rs` (Role: tui, Lines: 334)
- **Responsibility**: Core tui logic in src/ui/bundles.rs
- **Imports**: use anyhow :: Result , use console :: style , use inquire :: { Confirm , MultiSelect , Select } , use std :: path :: { Path , PathBuf } , use super :: TerminalUi , use crate :: bundle :: { delete_bundle_archive , delete_source_media_files , format_size , get_bundled_items_with_existing_sources , read_bundle_manifest , } , use crate :: db :: Database 
- **Public Functions & Signatures**:
  ```rust
  async fn manage_bundles_interactive (db : & Database) -> Result < () >
  async fn list_bundles_view (db : & Database) -> Result < () >
  async fn clean_bundled_sources_interactive (db : & Database) -> Result < () >
  async fn delete_bundles_interactive (db : & Database) -> Result < () >
  async fn prune_bundles_interactive (db : & Database) -> Result < () >
  ```

### `src/ui/card_dashboard/modal.rs` (Role: tui, Lines: 153)
- **Responsibility**: Core tui logic in src/ui/card_dashboard/modal.rs
- **Imports**: use ratatui :: { layout :: { Constraint , Direction , Layout , Rect } , style :: { Modifier , Style } , text :: { Line , Span } , widgets :: { Block , Borders , Clear , List , ListItem , Paragraph } , Frame , } , use super :: theme :: Theme , use crate :: dict :: LookupResult 
- **Types & Enums**:
  ```rust
  pub enum ActiveModal
  ```
- **Public Functions & Signatures**:
  ```rust
  fn render_candidate_modal (f : & mut Frame < '_ > , area : Rect , candidates : & [LookupResult] , selected_idx : usize ,)
  fn render_edit_reading_modal (f : & mut Frame < '_ > , area : Rect , buffer : & str)
  ```

### `src/ui/card_dashboard/panes.rs` (Role: tui, Lines: 367)
- **Responsibility**: Core tui logic in src/ui/card_dashboard/panes.rs
- **Imports**: use ratatui :: { layout :: { Constraint , Direction , Layout , Rect } , style :: { Modifier , Style } , text :: { Line , Span } , widgets :: { Block , Borders , Paragraph } , Frame , } , use super :: render :: DashboardRenderContext , use super :: theme :: Theme , use crate :: miner :: CandidateSentence , use crate :: ui :: helpers :: format_timestamp 
- **Public Functions & Signatures**:
  ```rust
  fn render_sentence_box (f : & mut Frame < '_ > , area : Rect , ctx : & DashboardRenderContext < '_ >)
  fn build_highlighted_sentence < 'a > (text : & 'a str , target : & 'a str) -> Line < 'a >
  fn render_split_panes (f : & mut Frame < '_ > , area : Rect , ctx : & DashboardRenderContext < '_ >)
  fn render_footer_toolbar (f : & mut Frame < '_ > , area : Rect)
  ```

### `src/ui/card_dashboard/render.rs` (Role: tui, Lines: 155)
- **Responsibility**: Core tui logic in src/ui/card_dashboard/render.rs
- **Imports**: use ratatui :: { layout :: { Constraint , Direction , Layout , Rect } , style :: { Modifier , Style } , text :: { Line , Span } , widgets :: { Block , Borders , Paragraph } , Frame , } , use super :: modal :: { render_candidate_modal , render_edit_reading_modal , ActiveModal } , use super :: panes :: { render_footer_toolbar , render_sentence_box , render_split_panes } , use super :: theme :: Theme , use crate :: dict :: LookupResult , use crate :: miner :: CandidateSentence 
- **Types & Enums**:
  ```rust
  pub struct DashboardRenderContext
  ```
- **Public Functions & Signatures**:
  ```rust
  fn render_dashboard (f : & mut Frame < '_ > , ctx : & DashboardRenderContext < '_ >)
  ```

### `src/ui/card_dashboard/tests.rs` (Role: tui, Lines: 317)
- **Responsibility**: Core tui logic in src/ui/card_dashboard/tests.rs
- **Imports**: use super :: * , use crate :: srt :: SubtitleSentence , use std :: path :: PathBuf 

### `src/ui/card_dashboard/theme.rs` (Role: tui, Lines: 18)
- **Responsibility**: Core tui logic in src/ui/card_dashboard/theme.rs
- **Imports**: use ratatui :: style :: Color 
- **Types & Enums**:
  ```rust
  pub struct Theme
  ```

### `src/ui/card_dashboard.rs` (Role: tui, Lines: 270)
- **Responsibility**: Core tui logic in src/ui/card_dashboard.rs
- **Imports**: use anyhow :: Result , use crossterm :: { event :: { self , Event , KeyCode , KeyEventKind , KeyModifiers } , execute , terminal :: { disable_raw_mode , enable_raw_mode , EnterAlternateScreen , LeaveAlternateScreen } , } , use ratatui :: { backend :: CrosstermBackend , Terminal } , use std :: io , use std :: path :: Path , use std :: time :: Duration , use crate :: dict :: LookupResult , use crate :: media :: MediaExtractor , use crate :: miner :: CandidateSentence , use modal :: ActiveModal , use render :: { render_dashboard , DashboardRenderContext } 
- **Types & Enums**:
  ```rust
  pub enum DashboardAction
  pub struct DashboardParams
  ```
- **Public Functions & Signatures**:
  ```rust
  fn run_card_dashboard (mut p : DashboardParams < '_ >) -> Result < DashboardAction >
  ```

### `src/ui/config_menu.rs` (Role: tui, Lines: 329)
- **Responsibility**: Core tui logic in src/ui/config_menu.rs
- **Imports**: use anyhow :: Result , use console :: Style , use inquire :: { Select , Text } 
- **Public Functions & Signatures**:
  ```rust
  fn show_config (cfg : & crate :: config :: AppConfig)
  fn configure_interactive (cfg : & mut crate :: config :: AppConfig) -> Result < () >
  ```

### `src/ui/explorer/actions.rs` (Role: tui, Lines: 26)
- **Responsibility**: Core tui logic in src/ui/explorer/actions.rs
- **Imports**: use anyhow :: Result , use std :: path :: Path , use std :: process :: Child , use crate :: db :: Database , use crate :: media :: MediaExtractor 
- **Public Functions & Signatures**:
  ```rust
  fn play_sentence_audio (video_path : & Path , start_ms : u64 , end_ms : u64) -> Option < Child >
  async fn mark_word_known (word : & str , db : & Database) -> Result < () >
  async fn mark_all_known (words : & [String] , db : & Database) -> Result < () >
  async fn mark_word_ignored (word : & str , db : & Database) -> Result < () >
  ```

### `src/ui/explorer/inspector_pane.rs` (Role: tui, Lines: 165)
- **Responsibility**: Core tui logic in src/ui/explorer/inspector_pane.rs
- **Imports**: use ratatui :: { layout :: Rect , style :: { Color , Modifier , Style as TuiStyle } , text :: { Line , Span } , widgets :: { Block , Borders , Paragraph } , Frame , } , use super :: render :: RenderExplorerContext 
- **Public Functions & Signatures**:
  ```rust
  fn render_word_inspector (frame : & mut Frame < '_ > , area : Rect , ctx : & RenderExplorerContext < '_ >)
  ```

### `src/ui/explorer/model.rs` (Role: tui, Lines: 223)
- **Responsibility**: Core tui logic in src/ui/explorer/model.rs
- **Imports**: use std :: collections :: HashSet , use crate :: nlp :: JapaneseTokenizer , use crate :: srt :: SubtitleSentence 
- **Types & Enums**:
  ```rust
  pub enum Tier
  pub struct UnknownWord
  pub struct ExplorerSentence
  pub enum SortOrder
  pub enum TierFilter
  pub struct SentenceStats
  ```
- **Public Functions & Signatures**:
  ```rust
  fn toggle (self) -> Self
  fn label (self) -> & 'static str
  fn next (self) -> Self
  fn label (self) -> & 'static str
  fn calculate_stats (sentences : & [ExplorerSentence]) -> SentenceStats
  fn sort_sentences (sentences : & mut [ExplorerSentence] , order : SortOrder)
  fn filter_sentences (sentences : & [ExplorerSentence] , tier_filter : TierFilter , search : & str ,) -> Vec < usize >
  fn refresh_sentence_unknowns (sentence : & mut ExplorerSentence , known_words : & HashSet < String > , ignored_words : & HashSet < String > ,)
  fn build_explorer_sentences (sentences : & [SubtitleSentence] , tokenizer : & JapaneseTokenizer , known_words : & HashSet < String > , ignored_words : & HashSet < String > ,) -> Vec < ExplorerSentence >
  ```

### `src/ui/explorer/render.rs` (Role: tui, Lines: 388)
- **Responsibility**: Core tui logic in src/ui/explorer/render.rs
- **Imports**: use ratatui :: { layout :: { Constraint , Direction , Layout , Rect } , style :: { Color , Modifier , Style as TuiStyle } , text :: { Line , Span } , widgets :: { Block , Borders , List , ListItem , Paragraph } , Frame , } , use std :: time :: Instant , use super :: model :: { ExplorerSentence , SentenceStats , SortOrder , Tier , TierFilter } , use crate :: dict :: LookupResult , use crate :: ui :: format_timestamp 
- **Types & Enums**:
  ```rust
  pub struct RenderExplorerContext
  ```
- **Public Functions & Signatures**:
  ```rust
  fn render_explorer (frame : & mut Frame < '_ > , ctx : RenderExplorerContext < '_ >)
  ```

### `src/ui/explorer/state.rs` (Role: tui, Lines: 324)
- **Responsibility**: Core tui logic in src/ui/explorer/state.rs
- **Imports**: use anyhow :: Result , use std :: collections :: { HashMap , HashSet } , use std :: process :: Child , use std :: time :: { Duration , Instant } , use crate :: dict :: { DictionaryService , LookupLimits , LookupResult } , use super :: actions , use super :: model :: { build_explorer_sentences , filter_sentences , refresh_sentence_unknowns , ExplorerSentence , SortOrder , TierFilter , } , use super :: ExplorerParams 
- **Types & Enums**:
  ```rust
  pub struct ExplorerController
  ```
- **Public Functions & Signatures**:
  ```rust
  fn new (p : ExplorerParams < 'a >) -> Self
  fn set_status (& mut self , text : impl Into < String >)
  fn visible_indices (& self) -> Vec < usize >
  fn current_sentence_and_word (& self) -> Option < (& ExplorerSentence , usize , String) >
  fn trigger_audio_for_current (& mut self)
  async fn ensure_active_word_cached (& mut self)
  fn toggle_selection (& mut self)
  fn toggle_all_in_current_sentence (& mut self)
  fn clear_selection (& mut self)
  fn build_selected_candidates (& self) -> Vec < crate :: miner :: CandidateSentence >
  async fn handle_mark_known (& mut self) -> Result < () >
  async fn handle_mark_all_known (& mut self) -> Result < () >
  async fn handle_mark_ignored (& mut self) -> Result < () >
  ```

### `src/ui/explorer/tests.rs` (Role: tui, Lines: 184)
- **Responsibility**: Core tui logic in src/ui/explorer/tests.rs
- **Imports**: use std :: collections :: HashSet , use super :: model :: * , use crate :: nlp :: JapaneseTokenizer , use crate :: srt :: SubtitleSentence 

### `src/ui/explorer.rs` (Role: tui, Lines: 258)
- **Responsibility**: Core tui logic in src/ui/explorer.rs
- **Imports**: use anyhow :: Result , use crossterm :: { event :: { self , Event , KeyCode , KeyEvent , KeyEventKind , KeyModifiers } , execute , terminal :: { disable_raw_mode , enable_raw_mode , EnterAlternateScreen , LeaveAlternateScreen } , } , use ratatui :: { backend :: CrosstermBackend , Terminal } , use std :: collections :: HashSet , use std :: io , use std :: path :: Path , use std :: time :: { Duration , Instant } , use crate :: config :: AppConfig , use crate :: db :: Database , use crate :: nlp :: JapaneseTokenizer , use crate :: srt :: SubtitleSentence , use model :: { calculate_stats , sort_sentences } , use render :: { render_explorer , RenderExplorerContext } , use state :: ExplorerController 
- **Types & Enums**:
  ```rust
  pub struct ExplorerParams
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn run_explorer (p : ExplorerParams < '_ >) -> Result < Vec < crate :: miner :: CandidateSentence > >
  ```

### `src/ui/helpers.rs` (Role: tui, Lines: 123)
- **Responsibility**: Core tui logic in src/ui/helpers.rs
- **Imports**: use console :: measure_text_width , use crossterm :: terminal , use std :: cmp :: Ordering , use unicode_segmentation :: UnicodeSegmentation , use unicode_width :: UnicodeWidthStr 
- **Public Functions & Signatures**:
  ```rust
  fn box_width () -> usize
  fn visual_width (s : & str) -> usize
  fn natural_cmp (left : & str , right : & str) -> Ordering
  fn format_word_with_reading (word : & str , reading : & str) -> String
  fn format_timestamp (start_ms : u64) -> String
  fn format_duration (d : std :: time :: Duration) -> String
  fn print_banner ()
  ```

### `src/ui/inspector.rs` (Role: tui, Lines: 347)
- **Responsibility**: Core tui logic in src/ui/inspector.rs
- **Imports**: use anyhow :: Result , use crossterm :: { event :: { self , Event , KeyCode , KeyEventKind } , execute , terminal :: { disable_raw_mode , enable_raw_mode , EnterAlternateScreen , LeaveAlternateScreen } , } , use ratatui :: { backend :: CrosstermBackend , layout :: { Constraint , Direction , Layout , Position } , style :: { Color , Modifier , Style as TuiStyle } , text :: { Line , Span } , widgets :: { Block , Borders , List , ListItem , Paragraph } , Terminal , } , use std :: io , use std :: path :: Path , use std :: time :: { Duration , Instant } , use crate :: ui :: format_timestamp , use crate :: ui :: helpers :: visual_width 
- **Types & Enums**:
  ```rust
  pub struct InspectSentencesParams
  ```
- **Public Functions & Signatures**:
  ```rust
  fn inspect_sentences (p : InspectSentencesParams < '_ >) -> Result < () >
  ```

### `src/ui/picker/render.rs` (Role: tui, Lines: 339)
- **Responsibility**: Core tui logic in src/ui/picker/render.rs
- **Imports**: use ratatui :: { layout :: { Constraint , Direction , Layout , Rect } , style :: { Color , Modifier , Style } , text :: { Line , Span } , widgets :: { Block , Borders , Paragraph } , Frame , } , use super :: state :: { FileSelectorState , SelectableFile } 
- **Public Functions & Signatures**:
  ```rust
  fn render_selector (frame : & mut Frame < '_ > , state : & FileSelectorState) -> usize
  ```

### `src/ui/picker/selector.rs` (Role: tui, Lines: 78)
- **Responsibility**: Core tui logic in src/ui/picker/selector.rs
- **Imports**: use anyhow :: { bail , Result } , use crossterm :: { cursor :: Show , event :: { self , Event , KeyEventKind } , execute , terminal :: { disable_raw_mode , enable_raw_mode , EnterAlternateScreen , LeaveAlternateScreen } , } , use ratatui :: { backend :: CrosstermBackend , Terminal } , use std :: io , use std :: path :: PathBuf , use std :: time :: Duration , use super :: render :: render_selector , use super :: state :: { FileSelectorState , SelectableFile , SelectorAction } 
- **Public Functions & Signatures**:
  ```rust
  fn run_file_selector (title : impl Into < String > , items : Vec < SelectableFile > , multi : bool ,) -> Result < Vec < PathBuf > >
  ```

### `src/ui/picker/state.rs` (Role: tui, Lines: 343)
- **Responsibility**: Core tui logic in src/ui/picker/state.rs
- **Imports**: use crossterm :: event :: { KeyCode , KeyEvent , KeyModifiers } , use std :: collections :: HashSet , use std :: path :: PathBuf 
- **Types & Enums**:
  ```rust
  pub enum SubtitleStatus
  pub struct SelectableFile
  pub enum SelectorAction
  pub enum CategoryFilter
  pub struct FileSelectorState
  ```
- **Public Functions & Signatures**:
  ```rust
  fn next (self) -> Self
  fn label (self) -> & 'static str
  fn matches (self , item : & SelectableFile) -> bool
  fn new (title : impl Into < String > , items : Vec < SelectableFile > , multi : bool) -> Self
  fn recompute_filter (& mut self)
  fn toggle_and_advance (& mut self)
  fn toggle_and_retreat (& mut self)
  fn ensure_cursor_visible (& mut self , visible_height : usize)
  fn handle_key (& mut self , key : KeyEvent , visible_height : usize) -> SelectorAction
  fn build_result (self) -> Vec < PathBuf >
  ```

### `src/ui/picker.rs` (Role: tui, Lines: 274)
- **Responsibility**: Core tui logic in src/ui/picker.rs
- **Imports**: use anyhow :: Result , use indicatif :: { ProgressBar , ProgressStyle } , use inquire :: Text , use std :: path :: { Path , PathBuf } , use walkdir :: WalkDir , pub use selector :: run_file_selector , pub use state :: { SelectableFile , SubtitleStatus } , use super :: helpers :: natural_cmp 
- **Public Functions & Signatures**:
  ```rust
  fn is_hidden_or_ignored_entry (entry : & walkdir :: DirEntry) -> bool
  fn discover_media_files (allowed_exts : & [& str] , spinner_msg : & str) -> Result < Vec < PathBuf > >
  fn select_media_file () -> Result < PathBuf >
  fn select_media_files () -> Result < Vec < PathBuf > >
  fn select_bundle_source_files () -> Result < Vec < PathBuf > >
  ```

### `src/ui/prompts.rs` (Role: tui, Lines: 83)
- **Responsibility**: Core tui logic in src/ui/prompts.rs
- **Imports**: use anyhow :: Result , use inquire :: Select , use super :: SessionMode 
- **Public Functions & Signatures**:
  ```rust
  fn select_session_mode (i1_count : usize , known_lines_count : usize) -> Result < SessionMode >
  fn ask_next_batch (next_batch : usize , total_batches : usize , remaining_lines : usize ,) -> Result < bool >
  fn select_sense (senses : & [String] , target_word : & str) -> Result < Option < String > >
  ```

### `src/ui/queue.rs` (Role: tui, Lines: 114)
- **Responsibility**: Core tui logic in src/ui/queue.rs
- **Imports**: use anyhow :: Result , use console :: style , use inquire :: { MultiSelect , Select } , use std :: path :: Path , use crate :: queue :: QueueEntry , use crate :: ui :: helpers :: format_timestamp 
- **Types & Enums**:
  ```rust
  pub enum QueueNoSubChoice
  ```
- **Public Functions & Signatures**:
  ```rust
  fn select_queue_entries (entries : & [QueueEntry]) -> Result < Vec < usize > >
  fn ask_no_subtitle_action (video_path : & Path , timestamp_ms : u64) -> Result < QueueNoSubChoice >
  fn select_target_word (sentence_text : & str , words : & [(String , String , bool)] ,) -> Result < Option < String > >
  ```

### `src/ui/tests.rs` (Role: tui, Lines: 126)
- **Responsibility**: Core tui logic in src/ui/tests.rs
- **Imports**: use super :: helpers :: natural_cmp , use super :: picker :: is_hidden_or_ignored_entry , use super :: picker :: state :: SubtitleStatus , use walkdir :: WalkDir 

### `src/ui.rs` (Role: tui, Lines: 125)
- **Responsibility**: Core tui logic in src/ui.rs
- **Imports**: pub use helpers :: * , pub use inspector :: InspectSentencesParams , use anyhow :: Result , use std :: path :: PathBuf 
- **Types & Enums**:
  ```rust
  pub enum SessionMode
  pub struct TerminalUi
  ```
- **Public Functions & Signatures**:
  ```rust
  fn print_banner ()
  fn select_media_file () -> Result < PathBuf >
  fn select_media_files () -> Result < Vec < PathBuf > >
  fn select_bundle_source_files () -> Result < Vec < PathBuf > >
  fn bootstrap_known_words (vocab_items : & [(String , usize , String)]) -> Result < Vec < String > >
  fn bootstrap_ignored_names (vocab_items : & [(String , usize , String)]) -> Result < Vec < String > >
  fn select_session_mode (i1_count : usize , known_lines_count : usize) -> Result < SessionMode >
  fn ask_next_batch (next_batch : usize , total_batches : usize , remaining_lines : usize ,) -> Result < bool >
  fn select_sense (senses : & [String] , target_word : & str) -> Result < Option < String > >
  fn inspect_sentences (p : inspector :: InspectSentencesParams < '_ >) -> Result < () >
  fn manage_known_words (words : & [(String , String)]) -> Result < Vec < String > >
  fn manage_ignored_words (words : & [(String , String)]) -> Result < Vec < String > >
  fn manage_mined_words (words : & [(String , String)]) -> Result < Vec < String > >
  fn show_config (cfg : & crate :: config :: AppConfig)
  async fn run_explorer (p : explorer :: ExplorerParams < '_ > ,) -> Result < Vec < crate :: miner :: CandidateSentence > >
  fn configure_interactive (cfg : & mut crate :: config :: AppConfig) -> Result < () >
  fn select_queue_entries (entries : & [crate :: queue :: QueueEntry]) -> Result < Vec < usize > >
  fn ask_no_subtitle_action (video_path : & std :: path :: Path , timestamp_ms : u64 ,) -> Result < queue :: QueueNoSubChoice >
  fn select_target_word (sentence_text : & str , words : & [(String , String , bool)] ,) -> Result < Option < String > >
  fn run_card_dashboard (p : card_dashboard :: DashboardParams < '_ > ,) -> Result < card_dashboard :: DashboardAction >
  ```

## 4. Execution Lifecycle Trace
1. **Startup**: Entrypoint parses CLI flags & dispatches command.
2. **Execution**: Core domain logic processes inputs and evaluates rules.
3. **Persistence / I/O**: Domain logic calls infra for disk/terminal I/O.
4. **Exit**: Graceful termination with standard exit codes.

## 5. Verification Commands
```bash
cargo build --release --target x86_64-unknown-linux-gnu
cargo test --all-targets
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```
