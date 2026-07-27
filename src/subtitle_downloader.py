import datetime
import difflib
import json
import pathlib
import re
import sys
from typing import Dict, List, Optional
from urllib.parse import urljoin, unquote
import httpx
from selectolax.parser import HTMLParser
from rich.console import Console
from rich.prompt import Prompt
from rich.table import Table

# Try importing CONFIG_DIR from project configuration
try:
    from src.config import CONFIG_DIR
except ImportError:
    CONFIG_DIR = pathlib.Path.home() / ".nihongo-miner"

BASE_URL = "https://subtitles.ajatt.top/"


class ShowEntry:
    """Represents a show (Anime or Drama) entry from the catalog."""

    def __init__(
        self,
        title: str,
        english_title: str,
        japanese_title: str,
        url: str,
        show_type: str,
    ) -> None:
        self.title: str = title
        self.english_title: str = english_title
        self.japanese_title: str = japanese_title
        self.url: str = url
        self.show_type: str = show_type

    def to_dict(self) -> Dict[str, str]:
        """Convert entry to dictionary for cache storage."""
        return {
            "title": self.title,
            "english_title": self.english_title,
            "japanese_title": self.japanese_title,
            "url": self.url,
            "show_type": self.show_type,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, str]) -> "ShowEntry":
        """Reconstruct entry from cached dictionary."""
        return cls(
            title=data.get("title", ""),
            english_title=data.get("english_title", ""),
            japanese_title=data.get("japanese_title", ""),
            url=data.get("url", ""),
            show_type=data.get("show_type", ""),
        )


class SubtitleFile:
    """Represents a single subtitle file entry (.srt, .ass, etc.) on a show page."""

    def __init__(self, filename: str, url: str) -> None:
        self.filename: str = filename
        self.url: str = url


def parse_filename_se(name: str) -> tuple[Optional[int], Optional[int]]:
    """Parse season and episode numbers from subtitle filename."""
    name = name.lower()
    
    # Extract pattern like S01E01
    se_match = re.search(r"s(\d+)e(\d+)", name)
    if se_match:
        return int(se_match.group(1)), int(se_match.group(2))
        
    # Episode patterns like ep01, ep 01, 第1話, 第01話
    ep_match = re.search(r"(?:ep|episode|第)\s*0*(\d+)", name)
    ep = int(ep_match.group(1)) if ep_match else None
    
    # Season patterns like s1, s01, season 1
    s_match = re.search(r"(?:s|season\s*)0*(\d+)", name)
    s = int(s_match.group(1)) if s_match else None
    
    # Clean standard video/codec/rip metadata keywords
    name_clean = re.sub(
        r"10bit|8bit|10-bit|8-bit|x264|x265|h264|h265|hi10", "", name
    )
    
    # Extract standalone numbers
    all_nums = re.findall(r"\d+", name_clean)
    clean_nums = []
    for num in all_nums:
        val = int(num)
        # Exclude common resolutions and year ranges
        if val not in {1080, 720, 480, 576, 2048, 1920, 1280} and not (
            1980 <= val <= 2030
        ):
            clean_nums.append(val)
            
    if ep is None and clean_nums:
        ep = clean_nums[0]
        
    return s, ep


def calculate_fuzzy_score(query: str, target: str) -> float:
    """Calculate a fuzzy match similarity score between a search query and a target string."""
    q_lower = query.lower().strip()
    t_lower = target.lower().strip()
    if not q_lower:
        return 0.0
    if q_lower == t_lower:
        return 100.0

    # Extract numbers from query
    q_nums = [int(n) for n in re.findall(r"\d+", q_lower)]

    # Parse target season & episode
    t_s, t_ep = parse_filename_se(t_lower)

    # Base text similarity (ignoring numbers)
    q_text = re.sub(r"\d+", "", q_lower).strip()
    t_text = re.sub(r"\d+", "", t_lower).strip()

    text_score = 0.0
    if q_text:
        if q_text in t_text:
            text_score += 10.0 + (len(q_text) / len(t_text))
        else:
            text_score += (
                difflib.SequenceMatcher(None, q_text, t_text).ratio() * 10.0
            )

    # Calculate numeric match score
    num_score = 0.0
    if q_nums:
        is_single_number = len(q_nums) == 1 and q_lower.isdigit()

        se_query_match = re.search(r"s(\d+)e(\d+)", q_lower)
        if se_query_match:
            q_s, q_ep = int(se_query_match.group(1)), int(
                se_query_match.group(2)
            )
            if t_s == q_s and t_ep == q_ep:
                num_score += 50.0
            elif t_s == q_s:
                num_score += 10.0
        else:
            for q_val in q_nums:
                if t_ep == q_val:
                    num_score += 40.0
                elif t_s == q_val:
                    if "s" in q_lower or "season" in q_lower:
                        num_score += 30.0
                    elif not is_single_number:
                        num_score += 5.0
                    else:
                        num_score -= 10.0

    whole_ratio = difflib.SequenceMatcher(None, q_lower, t_lower).ratio()
    return text_score + num_score + whole_ratio


def parse_range_selection(selection: str, max_val: int) -> List[int]:
    """Parse input selection strings like '1', '1,2,3', '1-5' into 0-based indices."""
    indices = set()
    parts = re.split(r"[\s,]+", selection.strip())
    for part in parts:
        if not part:
            continue
        if "-" in part:
            match = re.match(r"^(\d+)-(\d+)$", part)
            if match:
                start = int(match.group(1)) - 1
                end = int(match.group(2)) - 1
                if 0 <= start <= end < max_val:
                    for i in range(start, end + 1):
                        indices.add(i)
        else:
            if part.isdigit():
                val = int(part) - 1
                if 0 <= val < max_val:
                    indices.add(val)
    return sorted(list(indices))


class SubtitleDownloader:
    """Core class representing the downloader engine."""

    def __init__(self, cache_expiry_hours: int = 24) -> None:
        self.console: Console = Console()
        self.client: httpx.Client = httpx.Client(timeout=15.0)
        self.cache_expiry_hours: int = cache_expiry_hours
        self.cache_file: pathlib.Path = CONFIG_DIR / "subtitles_cache.json"

    def load_cached_shows(self) -> Optional[List[ShowEntry]]:
        """Load show list from cache file if still valid."""
        if not self.cache_file.exists():
            return None
        try:
            mtime = self.cache_file.stat().st_mtime
            age_hours = (datetime.datetime.now().timestamp() - mtime) / 3600.0
            if age_hours > self.cache_expiry_hours:
                self.console.print("[yellow]Cache expired, fetching fresh catalog...[/yellow]")
                return None

            with open(self.cache_file, "r", encoding="utf-8") as f:
                data = json.load(f)
                return [ShowEntry.from_dict(item) for item in data]
        except Exception as e:
            self.console.print(f"[red]Error reading cache: {e}[/red]")
            return None

    def save_cached_shows(self, shows: List[ShowEntry]) -> None:
        """Save show list to local cache folder."""
        try:
            self.cache_file.parent.mkdir(parents=True, exist_ok=True)
            with open(self.cache_file, "w", encoding="utf-8") as f:
                json.dump([show.to_dict() for show in shows], f, ensure_ascii=False, indent=2)
        except Exception as e:
            self.console.print(f"[yellow]Warning: Failed to save cache: {e}[/yellow]")

    def fetch_shows_from_url(self, url: str, show_type: str) -> List[ShowEntry]:
        """Fetch and parse index entries from a single page."""
        self.console.print(f"Fetching {show_type} catalog from {url}...")
        try:
            r = self.client.get(url)
            r.raise_for_status()
        except Exception as e:
            self.console.print(f"[red]Failed to fetch {url}: {e}[/red]")
            return []

        parser = HTMLParser(r.text)
        rows = parser.css("table.entries_table tbody tr")
        shows: List[ShowEntry] = []
        for row in rows:
            name_el = row.css_first("td.entry_name a")
            if not name_el:
                continue
            title = name_el.text().strip()
            relative_url = name_el.attributes.get("href", "")
            absolute_url = urljoin(BASE_URL, relative_url)

            eng_el = row.css_first("td.english_name")
            english_title = eng_el.text().strip() if eng_el else ""
            if english_title.lower() == "none":
                english_title = ""

            jap_el = row.css_first("td.japanese_name")
            japanese_title = jap_el.text().strip() if jap_el else ""
            if japanese_title.lower() == "none":
                japanese_title = ""

            shows.append(
                ShowEntry(
                    title=title,
                    english_title=english_title,
                    japanese_title=japanese_title,
                    url=absolute_url,
                    show_type=show_type,
                )
            )
        return shows

    def get_all_shows(self) -> List[ShowEntry]:
        """Load catalog from cache, fallback to fetching and saving to cache."""
        shows = self.load_cached_shows()
        if shows:
            return shows

        anime_shows = self.fetch_shows_from_url(urljoin(BASE_URL, "index.html"), "Anime")
        drama_shows = self.fetch_shows_from_url(urljoin(BASE_URL, "drama.html"), "Drama")

        all_shows = anime_shows + drama_shows
        if all_shows:
            self.save_cached_shows(all_shows)
            self.console.print(f"[green]Successfully loaded and cached {len(all_shows)} shows.[/green]")
        return all_shows

    def search_shows(self, query: str, shows: List[ShowEntry]) -> List[tuple[ShowEntry, float]]:
        """Filter show entries by title fuzzy match quality."""
        scored_shows = []
        for show in shows:
            score = max(
                calculate_fuzzy_score(query, show.title),
                calculate_fuzzy_score(query, show.english_title),
                calculate_fuzzy_score(query, show.japanese_title),
            )
            if score > 0.1:
                scored_shows.append((show, score))
        scored_shows.sort(key=lambda x: x[1], reverse=True)
        return scored_shows

    def display_shows(self, scored_shows: List[tuple[ShowEntry, float]]) -> None:
        """Print list of found shows in a neat table."""
        table = Table(title="Matching Shows", show_header=True, header_style="bold magenta")
        table.add_column("No.", justify="right", style="cyan", no_wrap=True)
        table.add_column("Title", style="white")
        table.add_column("English Name", style="dim white")
        table.add_column("Japanese Name", style="dim green")
        table.add_column("Type", style="yellow")
        table.add_column("Score", justify="right", style="magenta")

        for idx, (show, score) in enumerate(scored_shows[:15], 1):
            table.add_row(
                str(idx),
                show.title,
                show.english_title or "-",
                show.japanese_title or "-",
                show.show_type,
                f"{score:.2f}",
            )
        self.console.print(table)

    def fetch_subtitles_for_show(self, show: ShowEntry) -> List[SubtitleFile]:
        """Fetch and extract subtitle links from target show page."""
        self.console.print(f"Fetching subtitles for [bold cyan]{show.title}[/bold cyan]...")
        try:
            r = self.client.get(show.url)
            r.raise_for_status()
        except Exception as e:
            self.console.print(f"[red]Error fetching show page: {e}[/red]")
            return []

        parser = HTMLParser(r.text)
        subtitles: List[SubtitleFile] = []
        seen_urls = set()
        for a in parser.css("a"):
            href = a.attributes.get("href", "")
            if not href:
                continue
            if "raw.githubusercontent.com" in href or href.endswith(".srt") or href.endswith(".ass"):
                if href in seen_urls:
                    continue
                seen_urls.add(href)
                filename = a.text().strip()
                if not filename:
                    filename = unquote(href.split("/")[-1])
                subtitles.append(SubtitleFile(filename=filename, url=href))
        return subtitles

    def search_subtitles(self, query: str, subtitles: List[SubtitleFile]) -> List[tuple[SubtitleFile, float]]:
        """Score and sort subtitles based on fuzzy filename match quality."""
        scored_subs = []
        for sub in subtitles:
            score = calculate_fuzzy_score(query, sub.filename)
            if score > 0.0:
                scored_subs.append((sub, score))
        scored_subs.sort(key=lambda x: x[1], reverse=True)
        return scored_subs

    def display_subtitles(self, scored_subs: List[tuple[SubtitleFile, float]], show_score: bool = True) -> None:
        """Display subtitles list."""
        table = Table(title="Subtitles", show_header=True, header_style="bold green")
        table.add_column("No.", justify="right", style="cyan", no_wrap=True)
        table.add_column("Filename", style="white")
        if show_score:
            table.add_column("Match Score", justify="right", style="magenta")

        for idx, (sub, score) in enumerate(scored_subs[:30], 1):
            if show_score:
                table.add_row(str(idx), sub.filename, f"{score:.2f}")
            else:
                table.add_row(str(idx), sub.filename)

        self.console.print(table)
        if len(scored_subs) > 30:
            self.console.print(f"[dim]... and {len(scored_subs) - 30} more files (showing top 30) ...[/dim]")

    def download_subtitle(self, sub: SubtitleFile, dest_dir: pathlib.Path) -> bool:
        """Download and write subtitle to target file path."""
        dest_dir.mkdir(parents=True, exist_ok=True)
        dest_path = dest_dir / sub.filename
        if dest_path.exists():
            self.console.print(f"[yellow]Skipping {sub.filename} (already exists)[/yellow]")
            return True
        self.console.print(f"Downloading [green]{sub.filename}[/green]...")
        try:
            r = self.client.get(sub.url)
            r.raise_for_status()
            with open(dest_path, "wb") as f:
                f.write(r.content)
            self.console.print(f"[bold green]Saved to: {dest_path}[/bold green]")
            return True
        except Exception as e:
            self.console.print(f"[red]Error downloading {sub.filename}: {e}[/red]")
            return False

    def run(self) -> Optional[pathlib.Path]:
        """Interactive execution loop. Returns the downloaded file path if successful."""
        self.console.print("[bold yellow]AJATT Subtitle Downloader Utility[/bold yellow]\n")

        shows = self.get_all_shows()
        if not shows:
            self.console.print("[red]No shows found. Exiting.[/red]")
            return None

        # 1. Title Selection
        selected_show = None
        while not selected_show:
            query = Prompt.ask("\nEnter search query for anime/drama title (or 'q' to quit)")
            if query.lower() in ("q", "quit", "exit"):
                return None

            scored_shows = self.search_shows(query, shows)
            if not scored_shows:
                self.console.print("[yellow]No matching shows found. Try again.[/yellow]")
                continue

            self.display_shows(scored_shows)

            choice = Prompt.ask("Select a show by number (or press Enter to search again)")
            if not choice:
                continue
            if choice.isdigit() and 1 <= int(choice) <= len(scored_shows[:15]):
                selected_show = scored_shows[int(choice) - 1][0]
            else:
                self.console.print("[red]Invalid choice.[/red]")

        # 2. Subtitle extraction
        subtitles = self.fetch_subtitles_for_show(selected_show)
        if not subtitles:
            self.console.print("[yellow]No subtitles found for this show.[/yellow]")
            return None

        # 3. Subtitle selection & downloading
        while True:
            sub_query = Prompt.ask(
                "\nEnter fuzzy query to match subtitles (e.g. episode number, rip type, or press Enter for all)"
            )

            if sub_query:
                scored_subs = self.search_subtitles(sub_query, subtitles)
                show_score = True
            else:
                scored_subs = [(sub, 1.0) for sub in subtitles]
                show_score = False

            if not scored_subs:
                self.console.print("[yellow]No matching subtitles found. Try again.[/yellow]")
                continue

            self.display_subtitles(scored_subs, show_score=show_score)

            selection = Prompt.ask(
                "Select subtitle(s) to download by number (e.g. '1', '1,2,3', '1-5', or press Enter to search again)"
            )
            if not selection:
                continue

            indices = parse_range_selection(selection, len(scored_subs[:30]))
            if not indices:
                self.console.print("[red]Invalid selection or out of range.[/red]")
                continue

            dest_dir_input = Prompt.ask("Enter destination folder", default=str(CONFIG_DIR / "subtitles"))
            dest_dir = pathlib.Path(dest_dir_input).expanduser()

            downloaded_count = 0
            first_downloaded_path = None
            for idx in indices:
                sub = scored_subs[idx][0]
                if self.download_subtitle(sub, dest_dir):
                    if downloaded_count == 0:
                        first_downloaded_path = dest_dir / sub.filename
                    downloaded_count += 1

            self.console.print(f"[bold green]Finished downloading {downloaded_count} file(s).[/bold green]")
            if downloaded_count > 0:
                return first_downloaded_path
            return None


def main() -> None:
    """Script entry point."""
    try:
        downloader = SubtitleDownloader()
        downloader.run()
    except KeyboardInterrupt:
        print("\nOperation cancelled by user.")
        sys.exit(0)


if __name__ == "__main__":
    main()
