import pathlib
import re
import tempfile
import whisper
from rich import print
from rich.panel import Panel
from rich.table import Table

from src.database import create_db_and_tables
from src.miner import DictLookup, KnowledgeModel, MiningEngine, SubtitleParser, TextAnalyzer, WordFrequency


def run_card_audio_test() -> None:
    create_db_and_tables()

    media_dir = pathlib.Path.home() / ".nihongo-miner" / "media"
    pkg_dir = media_dir / "Hi10_Higurashi_no_Naku_Koro_ni_01_DVD_480p_Exiled_Destiny_ja"

    if not pkg_dir.exists():
        print(f"[bold red]Package directory not found:[/bold red] {pkg_dir}")
        return

    srt_files = list(pkg_dir.glob("*.srt"))
    if not srt_files:
        print("[bold red]No SRT file found in package![/bold red]")
        return
    srt_path = srt_files[0]

    audio_files = list(pkg_dir.glob("audio.*"))
    if not audio_files:
        print("[bold red]No audio file found in package![/bold red]")
        return
    audio_path = audio_files[0]

    print(f"[bold cyan]Loading Subtitle File:[/bold cyan] {srt_path.name}")
    lines = SubtitleParser().parse(srt_path)
    print(f"  Parsed {len(lines)} subtitle lines.")

    analyzer = TextAnalyzer()
    knowledge = KnowledgeModel()
    frequency = WordFrequency()

    engine = MiningEngine(analyzer, knowledge, frequency)
    print("  Evaluating $i+1$ candidate sentences...")
    candidates = engine.find_candidates(lines)

    if not candidates:
        print("[bold red]No candidate sentences found in session![/bold red]")
        return

    print(f"[bold green]Found {len(candidates)} $i+1$ candidate sentences![/bold green]\n")

    # Select top 3 candidates for testing
    top_candidates = candidates[:3]

    print("[bold yellow]Loading Whisper model ('base')...[/bold yellow]")
    whisper_model = whisper.load_model("base")

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = pathlib.Path(tmpdir)

        for idx, cand in enumerate(top_candidates, 1):
            ts = cand.sentence.timestamp
            if not ts or "-->" not in ts:
                continue

            start_ts, end_ts = [t.strip().replace(",", ".") for t in ts.split("-->")]

            def _ts_to_sec(ts_str: str) -> float:
                parts = ts_str.strip().replace(",", ".").split(":")
                return float(parts[0]) * 3600 + float(parts[1]) * 60 + float(parts[2])

            start_sec = max(0.0, _ts_to_sec(start_ts) - 0.75)
            raw_end_sec = _ts_to_sec(end_ts)
            trimmed_end_sec = raw_end_sec - 0.45
            end_sec = trimmed_end_sec
            dur_sec = max(0.2, end_sec - start_sec)

            clip_out = tmp_path / f"test_card_{idx}_{cand.unknown_word}.mp3"

            import ffmpeg
            (
                ffmpeg.input(str(audio_path), ss=f"{start_sec:.3f}")
                .output(str(clip_out), t=f"{dur_sec:.3f}", acodec="libmp3lame", q=4)
                .overwrite_output()
                .run(quiet=True)
            )

            # Transcribe extracted clip using Whisper
            res = whisper_model.transcribe(str(clip_out), language="ja")
            transcribed_text = res.get("text", "").strip()

            # Format comparison table
            table = Table(show_header=True, header_style="bold magenta")
            table.add_column("Property", style="cyan", width=22)
            table.add_column("Value / Output", style="white")

            clean_sub_text = re.sub(r"<[^>]+>", "", cand.sentence.text).strip()

            table.add_row("Target Word", f"[bold green]{cand.unknown_word}[/bold green]")
            table.add_row("SRT Subtitle Timing", f"{start_ts} --> {end_ts}")
            table.add_row("Padded Clip Duration", f"{dur_sec:.2f}s (Lead-in: -0.35s, Lead-out: +0.35s)")
            table.add_row("Original Subtitle Text", clean_sub_text)
            table.add_row("Whisper Transcribed Text", f"[bold yellow]{transcribed_text}[/bold yellow]")

            print(Panel(table, title=f"[bold white]Test Card #{idx}: {cand.unknown_word}[/bold white]"))
            print()


if __name__ == "__main__":
    run_card_audio_test()
