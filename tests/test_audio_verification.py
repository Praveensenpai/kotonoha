import os
import pathlib
import re
import subprocess
import tempfile
from rich import print


def parse_srt_timestamp(ts_str: str) -> float:
    """Converts SRT timestamp HH:MM:SS,mmm to seconds float."""
    ts_str = ts_str.strip().replace(",", ".")
    parts = ts_str.split(":")
    return float(parts[0]) * 3600 + float(parts[1]) * 60 + float(parts[2])


def extract_clip_current_method(audio_src: pathlib.Path, start_ts: str, end_ts: str, out_path: pathlib.Path) -> None:
    """Current method: uses string timestamps directly with ss=start_ts, to=end_ts."""
    cmd = [
        "ffmpeg", "-y",
        "-ss", start_ts.replace(",", "."),
        "-i", str(audio_src),
        "-to", end_ts.replace(",", "."),
        "-acodec", "libmp3lame",
        "-q:v", "4",
        str(out_path)
    ]
    subprocess.run(cmd, capture_output=True, text=True)


def extract_clip_padded_method(audio_src: pathlib.Path, start_sec: float, end_sec: float, out_path: pathlib.Path, pad_start: float = 0.35, pad_end: float = 0.35) -> None:
    """Improved method: applies start/end padding and uses accurate duration -t seeking."""
    p_start = max(0.0, start_sec - pad_start)
    p_end = end_sec + pad_end
    duration = p_end - p_start

    cmd = [
        "ffmpeg", "-y",
        "-ss", f"{p_start:.3f}",
        "-i", str(audio_src),
        "-t", f"{duration:.3f}",
        "-acodec", "libmp3lame",
        "-q:v", "4",
        str(out_path)
    ]
    subprocess.run(cmd, capture_output=True, text=True)


def get_audio_duration(file_path: pathlib.Path) -> float:
    """Retrieves audio duration in seconds using ffprobe."""
    cmd = [
        "ffprobe", "-v", "error",
        "-show_entries", "format=duration",
        "-of", "default=noprint_wrappers=1:nokey=1",
        str(file_path)
    ]
    res = subprocess.run(cmd, capture_output=True, text=True)
    try:
        return float(res.stdout.strip())
    except Exception:
        return 0.0


def run_verification() -> None:
    media_dir = pathlib.Path.home() / ".nihongo-miner" / "media"
    pkgs = [d for d in media_dir.iterdir() if d.is_dir()] if media_dir.exists() else []

    if not pkgs:
        print("[bold red]No media packages found in ~/.nihongo-miner/media/[/bold red]")
        return

    pkg = pkgs[0]
    print(f"[bold cyan]Using Episode Package:[/bold cyan] {pkg.name}")

    audio_file = pkg / "audio.opus"
    if not audio_file.exists():
        audio_files = list(pkg.glob("audio.*"))
        if not audio_files:
            print("[bold red]No audio file found in package![/bold red]")
            return
        audio_file = audio_files[0]

    srt_files = list(pkg.glob("*.srt"))
    if not srt_files:
        print("[bold red]No SRT subtitle file found in package![/bold red]")
        return
    srt_file = srt_files[0]

    print(f"[bold cyan]Reading Subtitle File:[/bold cyan] {srt_file.name}")
    srt_content = srt_file.read_text(encoding="utf-8", errors="ignore")

    pattern = re.compile(
        r"(\d+)\s*\n(\d{2}:\d{2}:\d{2}[,\.]\d{3})\s*-->\s*(\d{2}:\d{2}:\d{2}[,\.]\d{3})\s*\n(.*?)(?=\n\n|\Z)",
        re.DOTALL
    )

    matches = list(pattern.finditer(srt_content))
    if not matches:
        print("[bold red]No subtitle entries matched in SRT![/bold red]")
        return

    # Find candidate entries with substantial Japanese text
    sample_candidates = []
    for m in matches:
        text = re.sub(r"<[^>]+>", "", m.group(4)).strip()
        text = re.sub(r"[\r\n]+", " ", text)
        if len(text) >= 6 and any(ord(c) > 0x3000 for c in text):
            start_ts = m.group(2)
            end_ts = m.group(3)
            sample_candidates.append((start_ts, end_ts, text))
            if len(sample_candidates) >= 5:
                break

    print(f"\n[bold yellow]Testing {len(sample_candidates)} Subtitle Clips...[/bold yellow]\n")

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = pathlib.Path(tmpdir)

        # Check if whisper is installed
        whisper_model = None
        try:
            import whisper
            print("[bold green]Whisper is installed! Loading 'tiny' or 'base' model for verification...[/bold green]")
            whisper_model = whisper.load_model("tiny")
        except ImportError:
            print("[bold yellow]Whisper is not installed in current env (skipping transcription verification).[/bold yellow]")

        for idx, (start_ts, end_ts, text) in enumerate(sample_candidates, 1):
            start_sec = parse_srt_timestamp(start_ts)
            end_sec = parse_srt_timestamp(end_ts)
            orig_duration = end_sec - start_sec

            clip_curr = tmp_path / f"clip_{idx}_curr.mp3"
            clip_padded = tmp_path / f"clip_{idx}_padded.mp3"

            extract_clip_current_method(audio_file, start_ts, end_ts, clip_curr)
            extract_clip_padded_method(audio_file, start_sec, end_sec, clip_padded)

            dur_curr = get_audio_duration(clip_curr)
            dur_padded = get_audio_duration(clip_padded)

            print(f"[bold white]Clip #{idx}:[/bold white] \"{text}\"")
            print(f"  SRT Timing: {start_ts} --> {end_ts} (Expected dur: {orig_duration:.2f}s)")
            print(f"  [cyan]Current Method Dur:[/cyan] {dur_curr:.2f}s")
            print(f"  [green]Padded Method Dur: [/green] {dur_padded:.2f}s")

            if whisper_model:
                try:
                    res_curr = whisper_model.transcribe(str(clip_curr), language="ja")
                    res_pad = whisper_model.transcribe(str(clip_padded), language="ja")
                    txt_curr = res_curr.get("text", "").strip()
                    txt_pad = res_pad.get("text", "").strip()
                    print(f"  Whisper (Current): {txt_curr}")
                    print(f"  Whisper (Padded):  {txt_pad}")
                except Exception as e:
                    print(f"  Whisper error: {e}")

            print("-" * 60)


if __name__ == "__main__":
    run_verification()
