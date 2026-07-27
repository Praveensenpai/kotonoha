import pathlib
from typing import List

try:
    import tomllib  # Available in Python 3.11+
except ImportError:
    # Fallback for Python < 3.11 if needed, though project requires >=3.14
    import pip._vendor.tomli as tomllib  # type: ignore

# Base folder for configuration files
CONFIG_DIR = pathlib.Path.home() / ".nihongo-miner"


def migrate_legacy_data() -> None:
    """Migrates database and config from old locations to the new .nihongo-miner folder."""
    import shutil

    new_dir = pathlib.Path.home() / ".nihongo-miner"

    # 1. Migrate config.toml
    new_config = new_dir / "config.toml"
    if not new_config.exists():
        legacy_config_paths = [
            pathlib.Path.home() / ".anime-miner" / "config.toml",
            pathlib.Path.home() / "AppData" / "Local" / "nihongo-miner" / "config.toml",
            pathlib.Path.home() / "Library" / "Application Support" / "nihongo-miner" / "config.toml",
            pathlib.Path.home() / ".config" / "nihongo-miner" / "config.toml",
        ]
        for old_config in legacy_config_paths:
            if old_config.exists():
                try:
                    new_dir.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(old_config), str(new_config))
                    print(f"Migrated legacy configuration to {new_config}")
                    break
                except Exception:
                    pass

    # 2. Migrate database
    new_db = new_dir / "data.db"
    if not new_db.exists():
        legacy_db_paths = [
            pathlib.Path.home() / ".anime-miner" / "data.db",
            pathlib.Path("data.db"),
            pathlib.Path.home() / "AppData" / "Local" / "nihongo-miner" / "data.db",
            pathlib.Path.home() / "Library" / "Application Support" / "nihongo-miner" / "data.db",
            pathlib.Path.home() / ".local" / "share" / "nihongo-miner" / "data.db",
        ]
        for old_db in legacy_db_paths:
            if old_db.exists() and old_db.resolve() != new_db.resolve():
                try:
                    new_dir.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(str(old_db), str(new_db))
                    print(f"Migrated legacy database to {new_db}")
                    break
                except Exception:
                    pass


migrate_legacy_data()

LOCAL_CONFIG = pathlib.Path("config.toml")
CONFIG_FILE = LOCAL_CONFIG if LOCAL_CONFIG.exists() else CONFIG_DIR / "config.toml"

DEFAULT_FRONT_TEMPLATE = (
    "<style>\n"
    ".card {{\n"
    '  font-family: "Hiragino Mincho ProN", "Yu Mincho", "Noto Serif CJK JP", "Noto Serif JP", serif;\n'
    "  font-size: 32px;\n"
    "  text-align: center;\n"
    "  color: #ffffff;\n"
    "  background-color: #1e1d18;\n"
    "  padding: 30px 20px;\n"
    "  position: relative;\n"
    "}}\n"
    ".card-header {{\n"
    "  position: absolute;\n"
    "  top: 15px;\n"
    "  left: 15px;\n"
    "  display: flex;\n"
    "  gap: 4px;\n"
    "  font-family: sans-serif;\n"
    "  font-size: 13px;\n"
    "}}\n"
    ".badge {{\n"
    "  border: 1px solid rgba(255, 255, 255, 0.4);\n"
    "  border-radius: 4px;\n"
    "  padding: 1px 6px;\n"
    "  color: #e0e0e0;\n"
    "  background-color: rgba(255, 255, 255, 0.05);\n"
    "}}\n"
    ".sentence {{\n"
    "  font-size: 34px;\n"
    "  margin-top: 30px;\n"
    "  margin-bottom: 15px;\n"
    "  line-height: 1.6;\n"
    "}}\n"
    "ruby rt {{\n"
    "  font-size: 0.45em;\n"
    "  color: #d0d0d0;\n"
    "  font-weight: normal;\n"
    "}}\n"
    ".hide-furigana ruby rt {{\n"
    "  visibility: hidden;\n"
    "}}\n"
    ".target-word, .target-word rt {{\n"
    "  color: #888888;\n"
    "  opacity: 0.7;\n"
    "  font-weight: bold;\n"
    "}}\n"
    ".toggle-btn {{\n"
    "  background-color: #ffffff;\n"
    "  color: #000000;\n"
    "  border: none;\n"
    "  border-radius: 16px;\n"
    "  padding: 6px 18px;\n"
    "  font-size: 14px;\n"
    "  font-weight: 500;\n"
    "  cursor: pointer;\n"
    "  margin-top: 10px;\n"
    "}}\n"
    ".toggle-btn:hover {{\n"
    "  opacity: 0.9;\n"
    "}}\n"
    ".audio-container {{\n"
    "  margin-top: 10px;\n"
    "}}\n"
    "</style>\n"
    '<div class="card-header"><span class="badge">00064</span><span class="badge">jp1k</span></div>\n'
    '<div class="sentence hide-furigana">{furigana_sentence}</div>\n'
    '<button class="toggle-btn" onclick="this.previousElementSibling.classList.toggle(\'hide-furigana\')">Toggle Readings</button>\n'
    '<div class="audio-container">{audio}</div>'
)
DEFAULT_BACK_TEMPLATE = (
    "<style>\n"
    ".card {{\n"
    '  font-family: "Hiragino Mincho ProN", "Yu Mincho", "Noto Serif CJK JP", "Noto Serif JP", serif;\n'
    "  font-size: 28px;\n"
    "  text-align: center;\n"
    "  color: #ffffff;\n"
    "  background-color: #1e1d18;\n"
    "  padding: 20px;\n"
    "  position: relative;\n"
    "}}\n"
    ".card-header {{\n"
    "  position: absolute;\n"
    "  top: 15px;\n"
    "  left: 15px;\n"
    "  display: flex;\n"
    "  gap: 4px;\n"
    "  font-family: sans-serif;\n"
    "  font-size: 13px;\n"
    "}}\n"
    ".badge {{\n"
    "  border: 1px solid rgba(255, 255, 255, 0.4);\n"
    "  border-radius: 4px;\n"
    "  padding: 1px 6px;\n"
    "  color: #e0e0e0;\n"
    "  background-color: rgba(255, 255, 255, 0.05);\n"
    "}}\n"
    ".sentence {{\n"
    "  font-size: 32px;\n"
    "  margin-top: 25px;\n"
    "  margin-bottom: 15px;\n"
    "  line-height: 1.6;\n"
    "}}\n"
    "ruby rt {{\n"
    "  font-size: 0.45em;\n"
    "  color: #d0d0d0;\n"
    "  font-weight: normal;\n"
    "}}\n"
    ".target-word, .target-word rt {{\n"
    "  color: #888888;\n"
    "  opacity: 0.7;\n"
    "  font-weight: bold;\n"
    "}}\n"
    ".translation-details {{\n"
    "  margin: 10px 0;\n"
    "}}\n"
    ".translation-details summary {{\n"
    "  display: inline-block;\n"
    "  background-color: transparent;\n"
    "  color: #ffffff;\n"
    "  border: 1px solid rgba(255, 255, 255, 0.6);\n"
    "  border-radius: 4px;\n"
    "  padding: 4px 14px;\n"
    "  font-size: 13px;\n"
    "  cursor: pointer;\n"
    "  list-style: none;\n"
    "  user-select: none;\n"
    "}}\n"
    ".translation-details summary::-webkit-details-marker {{\n"
    "  display: none;\n"
    "}}\n"
    ".translation-box {{\n"
    "  font-size: 20px;\n"
    "  color: #dddddd;\n"
    "  margin-top: 10px;\n"
    "}}\n"
    ".word-box {{\n"
    "  border: 1px solid rgba(255, 255, 255, 0.15);\n"
    "  border-radius: 4px;\n"
    "  padding: 12px 16px;\n"
    "  margin: 15px 0;\n"
    "  text-align: left;\n"
    "  background-color: rgba(0, 0, 0, 0.15);\n"
    "}}\n"
    ".word-header {{\n"
    "  font-size: 26px;\n"
    "  margin-bottom: 8px;\n"
    "  color: #ffffff;\n"
    "  display: flex;\n"
    "  align-items: center;\n"
    "  gap: 8px;\n"
    "}}\n"
    ".definition {{\n"
    '  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;\n'
    "  font-size: 18px;\n"
    "  line-height: 1.5;\n"
    "  color: #d8d8d8;\n"
    "}}\n"
    ".image-accordion {{\n"
    "  margin-top: 15px;\n"
    "  text-align: left;\n"
    "}}\n"
    ".image-accordion summary {{\n"
    "  cursor: pointer;\n"
    "  font-size: 16px;\n"
    "  color: #cccccc;\n"
    "  user-select: none;\n"
    "  margin-bottom: 10px;\n"
    "}}\n"
    "</style>\n"
    '<div class="card-header"><span class="badge">00064</span><span class="badge">jp1k</span></div>\n'
    '<div class="sentence">{furigana_sentence}</div>\n'
    '<details class="translation-details">\n'
    '  <summary>Reveal English translation</summary>\n'
    '  <div class="translation-box">{definition}</div>\n'
    '</details>\n'
    '<div class="word-box">\n'
    '  <div class="word-header">{audio}<span><b>{word}</b>{reading_suffix}</span></div>\n'
    '  <div class="definition">{definition}</div>\n'
    "</div>\n"
    "<details class=\"image-accordion\" open>\n"
    "  <summary>▼ Image</summary>\n"
    '  <div style="text-align: center;">{image}</div>\n'
    "</details>\n"
    "{stats}"
)


class CardConfig:
    """Configuration class for Anki card generation."""

    def __init__(self) -> None:
        self.deck_name: str = "Japanese Mining"
        self.model_name: str = "Basic"
        self.front_template: str = DEFAULT_FRONT_TEMPLATE
        self.back_template: str = DEFAULT_BACK_TEMPLATE
        self.tags: List[str] = ["ai_mined"]
        self.media_dir: str = str(CONFIG_DIR / "media")
        self.load_config()

    def load_config(self) -> None:
        """Loads configuration from config.toml, creating it with defaults if it doesn't exist."""
        if not CONFIG_FILE.exists():
            self.save_defaults()
            return

        try:
            with open(CONFIG_FILE, "rb") as f:
                data = tomllib.load(f)

            anki_cfg = data.get("anki", {})
            self.deck_name = anki_cfg.get("deck_name", self.deck_name)
            self.model_name = anki_cfg.get("model_name", self.model_name)
            self.front_template = anki_cfg.get("front_template", self.front_template)
            self.back_template = anki_cfg.get("back_template", self.back_template)
            self.tags = anki_cfg.get("tags", self.tags)

            media_cfg = data.get("media", {})
            self.media_dir = media_cfg.get("media_dir", self.media_dir)

            try:
                # Validate template formatting
                self.front_template.format(
                    word="", reading="", reading_suffix="", sentence="",
                    furigana_sentence="", definition="", audio="", image="",
                    known_words="", unknown_words="", base_score="", adjusted_score="", stats=""
                )
            except Exception:
                self.front_template = DEFAULT_FRONT_TEMPLATE
                self.back_template = DEFAULT_BACK_TEMPLATE
                self.save_defaults()

            if "translation-details" not in self.back_template:
                self.front_template = DEFAULT_FRONT_TEMPLATE
                self.back_template = DEFAULT_BACK_TEMPLATE
                self.save_defaults()
        except Exception as e:
            print(
                f"[bold yellow]Warning:[/bold yellow] Failed to load configuration from {CONFIG_FILE}: {e}. Using defaults."
            )

    def save_defaults(self) -> None:
        """Saves default configuration file."""
        try:
            if CONFIG_FILE != pathlib.Path("config.toml"):
                CONFIG_DIR.mkdir(parents=True, exist_ok=True)
            default_toml = (
                "[anki]\n"
                f'deck_name = "{self.deck_name}"\n'
                f'model_name = "{self.model_name}"\n'
                f"tags = {self.tags}\n\n"
                "# Template variables available:\n"
                "# {word}            - Target Japanese word\n"
                "# {reading}         - Word reading/pronunciation\n"
                "# {reading_suffix}  - Helper that returns ' (reading)' if reading differs from word, otherwise empty\n"
                "# {sentence}        - Source Japanese sentence (plain text)\n"
                "# {furigana_sentence} - Source sentence with furigana <ruby> tags over kanji (HTML)\n"
                "# {definition}      - Dictionary definition\n"
                "# {audio}           - Sound play tag (e.g. [sound:abc.mp3]) if audio is present\n"
                "# {image}           - Image tag (e.g. <img src=...>) if image is present\n"
                "# {known_words}     - List of known words\n"
                "# {unknown_words}   - List of unknown words\n"
                "# {base_score}      - Raw sentence frequency/length score\n"
                "# {adjusted_score}  - Adjusted frequency/length score\n"
                "# {stats}           - The default formatted stats HTML block\n\n"
                f'front_template = """{self.front_template}"""\n'
                f'back_template = """{self.back_template}"""\n\n'
                "[media]\n"
                "# Optional: Directory where extracted media (audio/images) will be saved.\n"
                "# If empty, it defaults to a 'media' folder in the same directory as the subtitle file.\n"
                f'media_dir = "{self.media_dir}"\n'
            )
            with open(CONFIG_FILE, "w", encoding="utf-8") as f:
                f.write(default_toml)
        except Exception as e:
            print(
                f"[bold yellow]Warning:[/bold yellow] Failed to write default configuration: {e}"
            )


# Global instance of configuration
config = CardConfig()
