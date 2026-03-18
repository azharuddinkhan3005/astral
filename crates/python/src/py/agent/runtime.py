"""Agent runtime — orchestrates multi-step analysis pipelines."""

from __future__ import annotations

import asyncio
import json
import logging
import time
from dataclasses import dataclass, field
from typing import Any

import anthropic

# The native Rust extension.
import astral as _native

logger = logging.getLogger("astral.agent")

# ── Cost tracking ────────────────────────────────────────────────────────────


@dataclass
class CostLedger:
    """Tracks cumulative API costs across pipeline steps."""

    input_tokens: int = 0
    output_tokens: int = 0
    requests: int = 0
    _step_costs: list[dict[str, Any]] = field(default_factory=list)

    # Pricing per 1M tokens (batch discount already applied).
    _PRICING: dict[str, tuple[float, float]] = field(
        default_factory=lambda: {
            "opus": (2.50, 12.50),
            "sonnet": (1.50, 7.50),
            "haiku": (0.40, 2.00),
        },
        init=False,
        repr=False,
    )

    def record(self, step_name: str, input_tok: int, output_tok: int, num_requests: int) -> None:
        self.input_tokens += input_tok
        self.output_tokens += output_tok
        self.requests += num_requests
        self._step_costs.append(
            {
                "step": step_name,
                "input_tokens": input_tok,
                "output_tokens": output_tok,
                "requests": num_requests,
            }
        )

    def estimate_usd(self, model: str) -> float:
        key = "haiku"
        for k in self._PRICING:
            if k in model:
                key = k
                break
        inp, out = self._PRICING[key]
        return (self.input_tokens / 1_000_000) * inp + (self.output_tokens / 1_000_000) * out

    def summary(self, model: str) -> dict[str, Any]:
        return {
            "total_input_tokens": self.input_tokens,
            "total_output_tokens": self.output_tokens,
            "total_requests": self.requests,
            "estimated_cost_usd": round(self.estimate_usd(model), 4),
            "steps": list(self._step_costs),
        }


# ── Pipeline step descriptor ────────────────────────────────────────────────


@dataclass
class PipelineStep:
    """A single step in an analysis pipeline."""

    name: str
    analysis_mode: str
    file_filter: list[str] = field(default_factory=list)
    depends_on: list[str] = field(default_factory=list)


# ── Agent runtime ────────────────────────────────────────────────────────────

_POLL_INTERVAL = 10


class AgentRuntime:
    """Orchestrates multi-step analysis pipelines.

    Each step produces a batch of requests, submits them to the Anthropic
    Batch API, polls for completion, and feeds results into the next step.
    Steps that share no dependencies are dispatched in parallel.
    """

    def __init__(
        self,
        repo_path: str,
        config_json: str = "{}",
    ) -> None:
        self.repo_path = repo_path
        self.config_json = config_json
        self.analyser = _native.Analyser(config_json)
        self.client = anthropic.Anthropic()
        self.ledger = CostLedger()
        self.step_results: dict[str, str] = {}  # step_name -> results JSON

    # ── public API ───────────────────────────────────────────────────────

    def run_pipeline(self, steps: list[PipelineStep]) -> dict[str, Any]:
        """Execute a full pipeline (blocking). Returns the aggregated summary."""
        return asyncio.run(self._run_pipeline_async(steps))

    async def _run_pipeline_async(self, steps: list[PipelineStep]) -> dict[str, Any]:
        """Execute the pipeline, parallelising independent steps."""
        remaining = list(steps)
        completed: set[str] = set()

        while remaining:
            # Find steps whose dependencies are satisfied.
            ready = [s for s in remaining if all(d in completed for d in s.depends_on)]
            if not ready:
                unsatisfied = [s.name for s in remaining]
                raise RuntimeError(
                    f"Dependency deadlock: no runnable steps among {unsatisfied}"
                )

            # Dispatch ready steps in parallel.
            tasks = [self._dispatch_step(step) for step in ready]
            await asyncio.gather(*tasks)

            for step in ready:
                completed.add(step.name)
                remaining.remove(step)

        model = json.loads(self.config_json).get("model", "claude-haiku-4-5-20251001")
        return {
            "steps_completed": list(completed),
            "cost": self.ledger.summary(model),
            "results": dict(self.step_results),
        }

    async def _dispatch_step(self, step: PipelineStep) -> None:
        """Submit, poll, and aggregate a single pipeline step."""
        logger.info("Dispatching step: %s (mode=%s)", step.name, step.analysis_mode)

        # Build a per-step config by overriding analysis_mode.
        step_config = json.loads(self.config_json)
        step_config["analysis_mode"] = step.analysis_mode
        step_config_json = json.dumps(step_config)
        step_analyser = _native.Analyser(step_config_json)

        # Build requests.
        requests_json = step_analyser.build_requests(self.repo_path)
        requests = json.loads(requests_json)
        if not requests:
            logger.warning("Step %s produced no requests — skipping.", step.name)
            self.step_results[step.name] = "[]"
            return

        # Submit batch.
        batch = await asyncio.to_thread(
            self.client.messages.batches.create, requests=requests
        )
        batch_id = batch.id
        logger.info("Step %s: batch %s submitted (%d requests)", step.name, batch_id, len(requests))

        # Poll until complete.
        while True:
            status = await asyncio.to_thread(
                self.client.messages.batches.retrieve, batch_id
            )
            if status.processing_status == "ended":
                break
            await asyncio.sleep(_POLL_INTERVAL)

        # Retrieve results.
        result_lines: list[str] = []
        for result in self.client.messages.batches.results(batch_id):
            result_lines.append(result.model_dump_json())
        raw_jsonl = "\n".join(result_lines)

        # Aggregate.
        results_json = step_analyser.aggregate_results(raw_jsonl, self.repo_path)

        # Record costs (rough estimate from token counts).
        results = json.loads(results_json)
        input_tok = sum(len(r.get("analysis", "")) for r in results) // 4
        output_tok = sum(len(r.get("analysis", "")) for r in results) // 4
        self.ledger.record(step.name, input_tok, output_tok, len(requests))

        self.step_results[step.name] = results_json
        logger.info("Step %s: completed with %d results", step.name, len(results))

    def dispatch(self, step: PipelineStep) -> str:
        """Dispatch a single step synchronously. Returns results JSON."""
        asyncio.run(self._dispatch_step(step))
        return self.step_results.get(step.name, "[]")
