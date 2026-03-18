import { Command } from "commander";
import Anthropic from "@anthropic-ai/sdk";
import ora from "ora";
import chalk from "chalk";
import * as fs from "node:fs";
import * as path from "node:path";

// The napi binary is loaded from the package root at runtime.
// eslint-disable-next-line @typescript-eslint/no-var-requires
const { Analyser } = require("../../index.js") as {
  Analyser: new (configJson: string) => AnalyserBinding;
};

// ---------------------------------------------------------------------------
// Types mirroring the napi binding
// ---------------------------------------------------------------------------

interface AnalyserBinding {
  scan(repoPath: string): CodeChunk[];
  buildRequests(repoPath: string): string;
  aggregateResults(repoPath: string, jsonl: string): string;
  renderOutput(resultsJson: string, format: string): string;
}

interface CodeChunk {
  id: string;
  filePath: string;
  language: string;
  chunkType: string;
  name: string | null;
  content: string;
  startLine: number;
  endLine: number;
  imports: string[];
}

interface BatchRequest {
  custom_id: string;
  params: {
    model: string;
    max_tokens: number;
    system?: string;
    messages: { role: string; content: string }[];
  };
}

// ---------------------------------------------------------------------------
// Anthropic Batch API helpers
// ---------------------------------------------------------------------------

const POLL_INTERVAL_MS = 5_000;

async function submitBatch(
  client: Anthropic,
  requests: BatchRequest[]
): Promise<string> {
  const body = requests.map((r) => ({
    custom_id: r.custom_id,
    params: {
      model: r.params.model,
      max_tokens: r.params.max_tokens,
      system: r.params.system,
      messages: r.params.messages,
    },
  }));

  const batch = await client.messages.batches.create({ requests: body });
  return batch.id;
}

async function pollBatch(
  client: Anthropic,
  batchId: string,
  spinner: ReturnType<typeof ora>
): Promise<void> {
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const status = await client.messages.batches.retrieve(batchId);

    const counts = status.request_counts;
    const total =
      counts.processing +
      counts.succeeded +
      counts.errored +
      counts.canceled +
      counts.expired;
    const done = counts.succeeded + counts.errored + counts.canceled + counts.expired;

    spinner.text = `Batch ${batchId}: ${done}/${total} complete`;

    if (status.processing_status === "ended") {
      return;
    }

    await sleep(POLL_INTERVAL_MS);
  }
}

async function fetchResults(
  client: Anthropic,
  batchId: string
): Promise<string> {
  const lines: string[] = [];

  for await (const result of client.messages.batches.results(batchId)) {
    lines.push(JSON.stringify(result));
  }

  return lines.join("\n");
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ---------------------------------------------------------------------------
// CLI programme
// ---------------------------------------------------------------------------

const program = new Command();

program
  .name("astral")
  .description("Codebase analysis via Claude Batch API")
  .version("0.1.0");

program
  .command("analyse")
  .alias("analyze")
  .description("Analyse a repository using the Anthropic Batch API")
  .argument("<repo-path>", "Path to the repository to analyse")
  .option("-c, --config <path>", "Path to astral config JSON file")
  .option(
    "-o, --output <dir>",
    "Output directory for reports",
    "./astral-output"
  )
  .option(
    "-f, --format <formats>",
    "Comma-separated output formats (markdown,json,csv,html)",
    "markdown,json"
  )
  .option("--dry-run", "Scan and build requests without submitting", false)
  .action(async (repoPath: string, opts: AnalyseOptions) => {
    const configJson = opts.config
      ? fs.readFileSync(path.resolve(opts.config), "utf-8")
      : "{}";

    const analyser = new Analyser(configJson);
    const resolvedRepo = path.resolve(repoPath);

    // --- 1. Scan ---
    const scanSpinner = ora("Scanning repository...").start();
    const chunks = analyser.scan(resolvedRepo);
    scanSpinner.succeed(
      `Scanned ${chalk.bold(chunks.length)} code chunks`
    );

    if (chunks.length === 0) {
      console.log(chalk.yellow("No code chunks found. Exiting."));
      return;
    }

    // --- 2. Build requests ---
    const buildSpinner = ora("Building batch requests...").start();
    const requestsJson = analyser.buildRequests(resolvedRepo);
    const requests: BatchRequest[] = JSON.parse(requestsJson);
    buildSpinner.succeed(
      `Built ${chalk.bold(requests.length)} batch requests`
    );

    // --- Dry-run exit ---
    if (opts.dryRun) {
      console.log(chalk.cyan("\n--- Dry Run ---"));
      console.log(`Chunks:   ${chunks.length}`);
      console.log(`Requests: ${requests.length}`);
      console.log(`Model:    ${requests[0]?.params.model ?? "N/A"}`);

      const outPath = path.join(opts.output, "dry-run-requests.json");
      fs.mkdirSync(opts.output, { recursive: true });
      fs.writeFileSync(outPath, JSON.stringify(requests, null, 2));
      console.log(`Requests written to ${chalk.underline(outPath)}`);
      return;
    }

    // --- 3. Submit batch ---
    const apiKey = process.env["ANTHROPIC_API_KEY"];
    if (!apiKey) {
      console.error(
        chalk.red("ANTHROPIC_API_KEY environment variable is required.")
      );
      process.exit(1);
    }

    const client = new Anthropic({ apiKey });

    const submitSpinner = ora("Submitting batch to Anthropic...").start();
    const batchId = await submitBatch(client, requests);
    submitSpinner.succeed(`Batch submitted: ${chalk.bold(batchId)}`);

    // --- 4. Poll for completion ---
    const pollSpinner = ora("Waiting for batch completion...").start();
    await pollBatch(client, batchId, pollSpinner);
    pollSpinner.succeed("Batch processing complete");

    // --- 5. Fetch results ---
    const fetchSpinner = ora("Fetching results...").start();
    const rawJsonl = await fetchResults(client, batchId);
    fetchSpinner.succeed("Results fetched");

    // --- 6. Aggregate ---
    const aggSpinner = ora("Aggregating results...").start();
    const resultsJson = analyser.aggregateResults(resolvedRepo, rawJsonl);
    aggSpinner.succeed("Results aggregated");

    // --- 7. Render outputs ---
    const formats = opts.format.split(",").map((f) => f.trim());
    fs.mkdirSync(opts.output, { recursive: true });

    for (const fmt of formats) {
      const renderSpinner = ora(`Rendering ${fmt}...`).start();
      const rendered = analyser.renderOutput(resultsJson, fmt);
      const ext = fmt === "markdown" ? "md" : fmt;
      const outPath = path.join(opts.output, `astral-report.${ext}`);
      fs.writeFileSync(outPath, rendered);
      renderSpinner.succeed(`Written ${chalk.underline(outPath)}`);
    }

    console.log(chalk.green("\nAnalysis complete."));
  });

interface AnalyseOptions {
  config?: string;
  output: string;
  format: string;
  dryRun: boolean;
}

program.parse();
