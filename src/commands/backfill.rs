use anyhow::Result;
use console::style;

use crate::ai::{CardBatchInput, UnifiedAiService};
use crate::config::AppConfig;
use crate::db::cards::MinedCard;
use crate::db::Database;
use crate::dict::LookupResult;

fn build_batch_inputs<'a>(
    chunk: &'a [MinedCard],
    empty_lookup: &'a [LookupResult],
    empty_ctx: &'a [String],
) -> Vec<CardBatchInput<'a>> {
    chunk
        .iter()
        .enumerate()
        .map(|(idx, card)| CardBatchInput {
            card_index: idx,
            sentence: &card.sentence,
            target_word: &card.target_word,
            target_reading: &card.reading,
            candidates: empty_lookup,
            series_title: None,
            before_context: empty_ctx,
            after_context: empty_ctx,
        })
        .collect()
}

async fn translate_chunk(
    client: &reqwest::Client,
    cfg: &AppConfig,
    db: &Database,
    chunk: &[MinedCard],
) -> Result<usize> {
    let empty_lookup: Vec<LookupResult> = Vec::new();
    let empty_ctx: Vec<String> = Vec::new();
    let batch_inputs = build_batch_inputs(chunk, &empty_lookup, &empty_ctx);

    let results = UnifiedAiService::analyze_batch(client, cfg, &batch_inputs).await?;
    let mut updated = 0;

    for res in results {
        let Some(card) = chunk.get(res.card_index) else {
            continue;
        };
        if res.english_natural.is_none() && res.english_literal.is_none() {
            continue;
        }
        db.update_card_translations(
            card.id,
            res.english_natural.as_deref(),
            res.english_literal.as_deref(),
        )
        .await?;
        updated += 1;
    }

    Ok(updated)
}

pub async fn run_backfill_translations_command(cfg: &AppConfig, db: &Database) -> Result<()> {
    if !cfg.ai.enable_ai {
        println!(
            " ℹ AI is disabled in configuration. Enable [ai] in config to backfill translations."
        );
        return Ok(());
    }

    let missing = db.get_cards_missing_translations().await?;
    if missing.is_empty() {
        println!(" ✔ All mined cards already have translations.");
        return Ok(());
    }

    println!(
        " 🤖 Backfilling English translations for {} past mined card(s)...",
        missing.len()
    );

    let client = reqwest::Client::new();
    let batch_size = cfg.ai.ai_batch_size.max(1);
    let chunks: Vec<_> = missing.chunks(batch_size).collect();
    let total_chunks = chunks.len();
    let mut total_backfilled = 0;

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        if total_chunks > 1 {
            println!(
                " ⏳ Translating batch {}/{} ({} cards)...",
                chunk_idx + 1,
                total_chunks,
                chunk.len()
            );
        }

        match translate_chunk(&client, cfg, db, chunk).await {
            Ok(count) => total_backfilled += count,
            Err(e) => eprintln!(" ⚠️ Batch {}/{} failed: {e}", chunk_idx + 1, total_chunks),
        }
    }

    let report = format!(
        "✔ Successfully backfilled translations for {}/{} card(s)!",
        total_backfilled,
        missing.len()
    );
    println!(" {}", style(report).green().bold());

    if cfg.anki.enable_sync {
        println!(" 🔄 Syncing updated translations to Anki...");
        if let Err(e) = crate::anki::sync_to_anki(cfg, db).await {
            eprintln!(" ⚠️ Anki sync notice: {e}");
        }
    }

    Ok(())
}
