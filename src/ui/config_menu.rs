use anyhow::Result;
use console::Style;
use inquire::{Select, Text};

pub fn show_config(cfg: &crate::config::AppConfig) {
    let cyan = Style::new().cyan().bold();
    let yellow = Style::new().yellow().bold();
    let green = Style::new().green().bold();
    let dim = Style::new().dim();

    println!("\n📋  K O T O N O H A   C O N F I G U R A T I O N\n");
    println!(
        "  • Card Limit:            {}",
        cyan.apply_to(cfg.default_card_limit)
    );
    println!(
        "  • Max Cached Cards:      {}",
        cyan.apply_to(cfg.max_cached_cards)
    );
    println!(
        "  • Media Directory:       {}",
        dim.apply_to(cfg.media_dir.display())
    );
    println!(
        "  • Database Path:         {}",
        dim.apply_to(cfg.db_path.display())
    );
    println!(
        "  • Enable AI:             {}",
        if cfg.ai.enable_ai {
            green.apply_to("true")
        } else {
            yellow.apply_to("false")
        }
    );
    println!(
        "  • Gemini Model:          {}",
        cyan.apply_to(&cfg.ai.gemini_model)
    );
    let key_status = match cfg.ai.gemini_api_key.as_deref() {
        Some(k) if !k.trim().is_empty() && k != "YOUR_GEMINI_API_KEY_HERE" => {
            let mask_len = k.len().saturating_sub(6);
            format!("{}{} (Set)", &k[..3.min(k.len())], "*".repeat(mask_len))
        }
        _ => "Not set (Set GEMINI_API_KEY env var or in config)".to_string(),
    };
    println!("  • Gemini API Key:        {}", yellow.apply_to(key_status));
    println!(
        "  • Anki Sync Enabled:     {}",
        if cfg.anki.enable_sync {
            green.apply_to("true")
        } else {
            yellow.apply_to("false")
        }
    );
    println!(
        "  • AnkiConnect URL:       {}",
        dim.apply_to(&cfg.anki.connect_url)
    );
    println!(
        "  • Anki Deck Name:        {}",
        cyan.apply_to(&cfg.anki.deck_name)
    );
    println!(
        "  • Anki Model Name:       {}",
        dim.apply_to(&cfg.anki.model_name)
    );
    println!(
        "  • Max Definition Senses: {}",
        cyan.apply_to(cfg.dict.max_definition_senses)
    );
    println!(
        "  • Max Glosses / Sense:   {}",
        cyan.apply_to(cfg.dict.max_glosses_per_sense)
    );
    println!(
        "  • AI Batch Size:         {}",
        cyan.apply_to(cfg.ai.ai_batch_size)
    );
    println!(
        "  • AI Cache TTL:          {} min",
        cyan.apply_to(cfg.ai.ai_cache_ttl_minutes)
    );
    println!();
}

pub fn configure_interactive(cfg: &mut crate::config::AppConfig) -> Result<()> {
    let cyan = Style::new().cyan().bold();
    let green = Style::new().green().bold();

    loop {
        let key_display = match cfg.ai.gemini_api_key.as_deref() {
            Some(k) if !k.trim().is_empty() && k != "YOUR_GEMINI_API_KEY_HERE" => {
                format!("{}...", &k[..3.min(k.len())])
            }
            _ => "Not Set".to_string(),
        };

        let options = vec![
            format!(
                "🎴  Default Card Limit         [Current: {}]",
                cfg.default_card_limit
            ),
            format!(
                "💾  Max Cached Cards           [Current: {}]",
                cfg.max_cached_cards
            ),
            format!("🔑  Gemini API Key             [Current: {}]", key_display),
            format!(
                "🤖  Gemini Model               [Current: {}]",
                cfg.ai.gemini_model
            ),
            format!(
                "⚡  Enable AI Disambiguation   [Current: {}]",
                cfg.ai.enable_ai
            ),
            format!(
                "📦  Anki Deck Name             [Current: {}]",
                cfg.anki.deck_name
            ),
            format!(
                "🔌  AnkiConnect URL            [Current: {}]",
                cfg.anki.connect_url
            ),
            format!(
                "📖  Max Definition Senses      [Current: {}]",
                cfg.dict.max_definition_senses
            ),
            format!(
                "📝  Max Glosses per Sense      [Current: {}]",
                cfg.dict.max_glosses_per_sense
            ),
            format!(
                "🔢  AI Batch Size              [Current: {}]",
                cfg.ai.ai_batch_size
            ),
            format!(
                "⌛  AI Cache TTL (minutes)     [Current: {} min]",
                cfg.ai.ai_cache_ttl_minutes
            ),
            "🔄  Reset to Default Values".to_string(),
            "💾  Save & Exit".to_string(),
        ];

        let choice = Select::new("Select configuration option to edit:", options)
            .with_page_size(12)
            .prompt()?;

        if choice.contains("Default Card Limit") {
            let input = Text::new("Enter default card limit per session:")
                .with_default(&cfg.default_card_limit.to_string())
                .prompt()?;
            if let Ok(num) = input.trim().parse::<usize>() {
                cfg.default_card_limit = num;
                println!(" ✔ Card limit set to {}", cyan.apply_to(num));
            }
        } else if choice.contains("Max Cached Cards") {
            let input = Text::new("Enter maximum cached cards to keep in media dir (0 to disable cleanup):")
                .with_default(&cfg.max_cached_cards.to_string())
                .prompt()?;
            if let Ok(num) = input.trim().parse::<usize>() {
                cfg.max_cached_cards = num;
                println!(" ✔ Max cached cards set to {}", cyan.apply_to(num));
            }
        } else if choice.contains("Gemini API Key") {
            let current_key = cfg.ai.gemini_api_key.clone().unwrap_or_default();
            let input = Text::new("Enter Gemini API key:")
                .with_default(&current_key)
                .prompt()?;
            let trimmed = input.trim();
            if !trimmed.is_empty() {
                cfg.ai.gemini_api_key = Some(trimmed.to_string());
                println!(" ✔ Gemini API key updated.");
            }
        } else if choice.contains("Gemini Model") {
            let models = vec![
                "gemini-3.5-flash-lite",
                "gemini-2.5-flash",
                "gemini-2.5-pro",
                "✍ Enter custom model",
            ];
            let selected_model = Select::new("Select Gemini model:", models).prompt()?;
            if selected_model.contains("custom model") {
                let custom = Text::new("Enter custom model name:").prompt()?;
                if !custom.trim().is_empty() {
                    cfg.ai.gemini_model = custom.trim().to_string();
                }
            } else {
                cfg.ai.gemini_model = selected_model.to_string();
            }
            println!(" ✔ Model set to {}", cyan.apply_to(&cfg.ai.gemini_model));
        } else if choice.contains("Enable AI Disambiguation") {
            cfg.ai.enable_ai = !cfg.ai.enable_ai;
            println!(
                " ✔ AI Disambiguation {}",
                if cfg.ai.enable_ai {
                    green.apply_to("Enabled")
                } else {
                    cyan.apply_to("Disabled")
                }
            );
        } else if choice.contains("Anki Deck Name") {
            let input = Text::new("Enter Anki deck name:")
                .with_default(&cfg.anki.deck_name)
                .prompt()?;
            if !input.trim().is_empty() {
                cfg.anki.deck_name = input.trim().to_string();
                println!(" ✔ Anki deck set to {}", cyan.apply_to(&cfg.anki.deck_name));
            }
        } else if choice.contains("AnkiConnect URL") {
            let input = Text::new("Enter AnkiConnect URL:")
                .with_default(&cfg.anki.connect_url)
                .prompt()?;
            if !input.trim().is_empty() {
                cfg.anki.connect_url = input.trim().to_string();
            }
        } else if choice.contains("Max Definition Senses") {
            let input = Text::new("Enter max definition senses:")
                .with_default(&cfg.dict.max_definition_senses.to_string())
                .prompt()?;
            if let Ok(num) = input.trim().parse::<usize>() {
                cfg.dict.max_definition_senses = num;
            }
        } else if choice.contains("Max Glosses per Sense") {
            let input = Text::new("Enter max glosses per sense:")
                .with_default(&cfg.dict.max_glosses_per_sense.to_string())
                .prompt()?;
            if let Ok(num) = input.trim().parse::<usize>() {
                cfg.dict.max_glosses_per_sense = num;
            }
        } else if choice.contains("AI Batch Size") {
            let input = Text::new("Enter AI batch size (cards per Gemini request):")
                .with_default(&cfg.ai.ai_batch_size.to_string())
                .prompt()?;
            if let Ok(num) = input.trim().parse::<usize>() {
                cfg.ai.ai_batch_size = num.max(1);
                println!(
                    " ✔ AI batch size set to {}",
                    cyan.apply_to(cfg.ai.ai_batch_size)
                );
            }
        } else if choice.contains("AI Cache TTL") {
            let input = Text::new("Enter AI analysis cache TTL in minutes (0 to disable cache):")
                .with_default(&cfg.ai.ai_cache_ttl_minutes.to_string())
                .prompt()?;
            if let Ok(num) = input.trim().parse::<usize>() {
                cfg.ai.ai_cache_ttl_minutes = num;
                println!(" ✔ AI Cache TTL set to {} minutes", cyan.apply_to(num));
            }
        } else if choice.contains("Reset to Default Values") {
            *cfg = crate::config::AppConfig::default();
            println!(" 🔄 Config reset to default values.");
        } else if choice.contains("Save & Exit") {
            cfg.save()?;
            println!(" ✨ Configuration saved to ~/.config/kotonoha/config.toml!");
            break;
        }
    }
    Ok(())
}
