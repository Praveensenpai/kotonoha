pub mod ai_analysis_cache;
pub mod all_candidates_cache;
pub mod bundled_media;
pub mod dictionary_cache;
pub mod ignored_words;
pub mod known_words;
pub mod mined_cards;
pub mod offline_terms;

pub use ai_analysis_cache::Entity as AiAnalysisCache;
pub use all_candidates_cache::Entity as AllCandidatesCache;
pub use bundled_media::Entity as BundledMedia;
pub use dictionary_cache::Entity as DictionaryCache;
pub use ignored_words::Entity as IgnoredWords;
pub use known_words::Entity as KnownWords;
pub use mined_cards::Entity as MinedCards;
pub use offline_terms::Entity as OfflineTerms;
