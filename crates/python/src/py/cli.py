"""astral CLI — powered by Typer, Rich, and the Anthropic SDK."""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path
from typing import Optional

import anthropic
import typer
from rich.console import Console
from rich.panel import Panel
from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn, TimeElapsedColumn

# The native extension is importable as "astral" once the wheel is installed.
import astral as _native

app = typer.Typer(
    name="astral",
    help="Codebase analysis via Claude Batch API.",
    add_completion=False,
)
console = Console()

# ── helpers ──────────────────────────────────────────────────────────────────

_POLL_INTERVAL = 10  # seconds between batch status checks


def _load_config(config_path: Optional[Path]) -> str:
    """Return the config JSON string, or '{}' for defaults."""
    if config_path is None:
        return "{}"
    text = config_path.read_text()
    # Validate it parses as JSON before handing to Rust.
    json.loads(text)
    return text


def _write_output(content: str, output_path: Optional[Path]) -> None:
    if output_path:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(content)
        console.print(f"[green]Output written to {output_path}[/green]")
    else:
        console.print(content)


# ── commands ─────────────────────────────────────────────────────────────────


@app.command()
def analyse(
    repo: str = typer.Argument(..., help="Path to the repository to analyse."),
    config: Optional[Path] = typer.Option(
        None,
        "--config",
        "-c",
        help="Path to an astral config JSON file.",
    ),
    output: Optional[Path] = typer.Option(
        None,
        "--output",
        "-o",
        help="File path to write the rendered output.",
    ),
    fmt: str = typer.Option(
        "markdown",
        "--format",
        "-f",
        help="Output format: markdown, json, csv, html.",
    ),
    dry_run: bool = typer.Option(
        False,
        "--dry-run",
        help="Scan and build requests but do not submit the batch.",
    ),
) -> None:
    """Analyse a codebase via the Anthropic Batch API."""

    config_json = _load_config(config)
    analyser = _native.Analyser(config_json)

    # ── Step 1: Scan ────────────────────────────────────────────────────
    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Scanning repository..."),
        transient=True,
    ) as progress:
        progress.add_task("scan", total=None)
        chunks = analyser.scan(repo)

    console.print(
        Panel(
            f"[bold]{len(chunks)}[/bold] code chunks discovered across the repository.",
            title="Scan complete",
            border_style="green",
        )
    )

    if not chunks:
        console.print("[yellow]No code chunks found. Nothing to do.[/yellow]")
        raise typer.Exit()

    # ── Step 2: Build requests ──────────────────────────────────────────
    requests_json = analyser.build_requests(repo)
    requests = json.loads(requests_json)

    console.print(
        Panel(
            f"[bold]{len(requests)}[/bold] batch requests prepared.",
            title="Requests built",
            border_style="cyan",
        )
    )

    if dry_run:
        console.print("[yellow]Dry-run mode — skipping batch submission.[/yellow]")
        console.print_json(requests_json)
        raise typer.Exit()

    # ── Step 3: Submit batch ────────────────────────────────────────────
    client = anthropic.Anthropic()

    console.print("[bold blue]Submitting batch to the Anthropic API...[/bold blue]")
    batch = client.messages.batches.create(requests=requests)
    batch_id = batch.id
    console.print(f"[green]Batch submitted:[/green] {batch_id}")

    # ── Step 4: Poll for completion ─────────────────────────────────────
    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Waiting for batch to complete..."),
        BarColumn(),
        TimeElapsedColumn(),
    ) as progress:
        task = progress.add_task("poll", total=None)
        while True:
            status = client.messages.batches.retrieve(batch_id)
            if status.processing_status == "ended":
                break
            progress.update(task, description=f"[bold blue]Status: {status.processing_status}")
            time.sleep(_POLL_INTERVAL)

    console.print(
        Panel(
            f"Batch [bold]{batch_id}[/bold] completed.",
            title="Batch finished",
            border_style="green",
        )
    )

    # ── Step 5: Retrieve results ────────────────────────────────────────
    console.print("[bold blue]Retrieving results...[/bold blue]")
    result_lines: list[str] = []
    for result in client.messages.batches.results(batch_id):
        result_lines.append(result.model_dump_json())
    raw_jsonl = "\n".join(result_lines)

    # ── Step 6: Aggregate ───────────────────────────────────────────────
    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Aggregating results..."),
        transient=True,
    ) as progress:
        progress.add_task("aggregate", total=None)
        results_json = analyser.aggregate_results(raw_jsonl, repo)

    results = json.loads(results_json)
    succeeded = sum(1 for r in results if r.get("status") == "succeeded")
    console.print(
        Panel(
            f"[bold]{succeeded}[/bold]/{len(results)} chunks analysed successfully.",
            title="Aggregation complete",
            border_style="green",
        )
    )

    # ── Step 7: Render output ───────────────────────────────────────────
    rendered = analyser.render_output(results_json, fmt)
    _write_output(rendered, output)


@app.command()
def scan(
    repo: str = typer.Argument(..., help="Path to the repository to scan."),
    config: Optional[Path] = typer.Option(
        None,
        "--config",
        "-c",
        help="Path to an astral config JSON file.",
    ),
) -> None:
    """Scan a repository and print discovered code chunks (no API calls)."""

    config_json = _load_config(config)
    analyser = _native.Analyser(config_json)
    chunks = analyser.scan(repo)

    console.print(
        Panel(
            f"[bold]{len(chunks)}[/bold] code chunks found.",
            title="Scan results",
            border_style="green",
        )
    )
    for chunk in chunks:
        name = chunk.get("name") or "<anonymous>"
        console.print(
            f"  [cyan]{chunk['file_path']}[/cyan]  "
            f"[dim]{chunk['chunk_type']}[/dim]  "
            f"[bold]{name}[/bold]  "
            f"L{chunk['start_line']}-{chunk['end_line']}"
        )


if __name__ == "__main__":
    app()
