import pathlib
import re
import subprocess
import tempfile
import whisper
from rich import print
from rich.table import Table


def main() -> None:
    print("[bold green]Loading Whisper 'large-v3' model...[/bold green]")
    model = whisper.load_model("large-v3")

    media_dir = pathlib.Path.home() / ".nihongo-miner" / "media"
    pkg_dir = media_dir / "Hi10_Higurashi_no_Naku_Koro_ni_01_DVD_480p_Exiled_Destiny_ja"
    audio_path = pkg_dir / "audio.opus"
    srt_path = pkg_dir / "Hi10_Higurashi_no_Naku_Koro_ni_01_DVD_480p_Exiled_Destiny_ja.srt"

    srt_text = srt_path.read_text(encoding="utf-8", errors="ignore")

    pattern = re.compile(
        r"(\d+)\s*\n(\d{2}:\d{2}:\d{2}[,\.]\d{3})\s*-->\s*(\d{2}:\d{2}:\d{2}[,\.]\d{3})\s*\n(.*?)(?=\n\n|\Z)",
        re.DOTALL,
    )

    def ts_to_sec(ts_str: str) -> float:
        parts = ts_str.strip().replace(",", ".").split(":")
        return float(parts[0]) * 3600 + float(parts[1]) * 60 + float(parts[2])

    matches = list(pattern.finditer(srt_text))

    test_indices = [11, 12, 13, 14, 15, 16]

    print("\n[bold yellow]=== WHISPER LARGE-V3 AUDIO CUTOFF VERIFICATION ===[/bold yellow]\n")

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = pathlib.Path(tmpdir)

        for idx in test_indices:
            if idx >= len(matches):
                continue
            m = matches[idx]
            sub_id = m.group(1)
            start_ts = m.group(2)
            end_ts = m.group(3)
            raw_text = re.sub(r"<[^>]+>", "", m.group(4)).strip().replace("\n", " ")

            start_sec = ts_to_sec(start_ts)
            end_sec = ts_to_sec(end_ts)

            next_start_sec = None
            if idx + 1 < len(matches):
                next_start_sec = ts_to_sec(matches[idx + 1].group(2))

            print(f"[bold cyan]Subtitle #{sub_id}:[/bold cyan] \"{raw_text}\"")
            print(f"  SRT Window: {start_ts} --> {end_ts} (Duration: {end_sec - start_sec:.2f}s)")
            if next_start_sec:
                print(f"  Next Subtitle #{matches[idx+1].group(1)} Starts At: {matches[idx+1].group(2)} (Gap: {next_start_sec - end_sec:.2f}s)")

            table = Table(show_header=True, header_style="bold magenta")
            table.add_column("Setting / Trim Option", style="cyan", width=30)
            table.add_column("Clip Dur", style="white", width=10)
            table.add_column("Whisper Large-V3 Transcription", style="green")

            trim_configs = [
                ("Raw SRT (0.0s / 0.0s)", 0.0, 0.0),
                ("Lead-in -0.75s / End 0.0s", 0.75, 0.0),
                ("Lead-in -0.75s / End -0.45s", 0.75, -0.45),
                ("Lead-in -0.75s / End -0.75s", 0.75, -0.75),
                ("Lead-in -0.75s / End -1.00s", 0.75, -1.00),
            ]

            for label, lead_in, lead_out in trim_configs:
                p_start = max(0.0, start_sec - lead_in)
                p_end = max(p_start + 0.2, end_sec + lead_out)
                if next_start_sec and next_start_sec > p_start and lead_out >= 0:
                    p_end = min(p_end, next_start_sec - 0.05)
                dur = p_end - p_start

                clip_file = tmp_path / f"sub_{sub_id}_{lead_in}_{lead_out}.mp3"
                cmd = [
                    "ffmpeg", "-y",
                    "-ss", f"{p_start:.3f}",
                    "-i", str(audio_path),
                    "-t", f"{dur:.3f}",
                    "-acodec", "libmp3lame",
                    "-q:v", "4",
                    str(clip_file),
                ]
                subprocess.run(cmd, capture_output=True, text=True)

                res = model.transcribe(str(clip_file), language="ja")
                trans = res.get("text", "").strip()
                table.add_row(label, f"{dur:.2f}s", trans)

            print(table)
            print("-" * 75)


if __name__ == "__main__":
    main()
