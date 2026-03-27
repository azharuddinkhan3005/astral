use anyhow::{Context, Result, bail};
use astral_core::batch_builder::estimate_cost;
use astral_core::outputs;
use astral_core::{Config, CoreAnalyser};
use clap::{Parser, Subcommand};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "astral",
    about = "Analyse any codebase with Claude — at scale, at 50% cost",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyse a codebase via Claude Batch API
    Analyse {
        /// Path to the repository to analyse
        repo_path: String,

        /// Config file path
        #[arg(short, long, default_value = "astral.config.json")]
        config: String,

        /// Output formats (comma-separated: markdown,json,html,sarif,jsonl,csv,vector)
        #[arg(short, long, default_value = "markdown,json")]
        output: String,

        /// Output directory
        #[arg(long, default_value = "./astral-output")]
        output_dir: String,

        /// Show cost estimate without submitting to the API
        #[arg(long)]
        dry_run: bool,
    },

    /// Scan a repo and show chunk summary (no API call)
    Scan {
        /// Path to the repository
        repo_path: String,

        /// Config file path
        #[arg(short, long, default_value = "astral.config.json")]
        config: String,
    },

    /// Aggregate results from a Batch API JSONL file
    Aggregate {
        /// Path to the repository (needed to re-scan chunks for ID mapping)
        repo_path: String,

        /// Path to the JSONL results file from Batch API
        jsonl_path: String,

        /// Config file path
        #[arg(short, long, default_value = "astral.config.json")]
        config: String,

        /// Output formats (comma-separated)
        #[arg(short, long, default_value = "markdown,json")]
        output: String,

        /// Output directory
        #[arg(long, default_value = "./astral-output")]
        output_dir: String,
    },
}

fn load_config(path: &str) -> Result<Config> {
    if std::path::Path::new(path).exists() {
        Config::from_file(path).context("Failed to load config file")
    } else {
        Ok(Config::default())
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyse {
            repo_path,
            config,
            output,
            output_dir,
            dry_run,
        } => cmd_analyse(&repo_path, &config, &output, &output_dir, dry_run),

        Commands::Scan { repo_path, config } => cmd_scan(&repo_path, &config),

        Commands::Aggregate {
            repo_path,
            jsonl_path,
            config,
            output,
            output_dir,
        } => cmd_aggregate(&repo_path, &jsonl_path, &config, &output, &output_dir),
    }
}

fn cmd_scan(repo_path: &str, config_path: &str) -> Result<()> {
    let config = load_config(config_path)?;
    let analyser = CoreAnalyser::new(config);

    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}")?);
    pb.set_message("Scanning repository...");
    pb.enable_steady_tick(Duration::from_millis(80));

    let chunks = analyser.scan(repo_path)?;
    pb.finish_and_clear();

    // Group by file
    let mut by_file: std::collections::BTreeMap<&str, Vec<&astral_core::CodeChunk>> =
        std::collections::BTreeMap::new();
    for chunk in &chunks {
        by_file.entry(&chunk.file_path).or_default().push(chunk);
    }

    let funcs = chunks
        .iter()
        .filter(|c| c.chunk_type == astral_core::ChunkType::Function)
        .count();
    let classes = chunks
        .iter()
        .filter(|c| c.chunk_type == astral_core::ChunkType::Class)
        .count();

    println!(
        "\n{}  {} chunks across {} files\n",
        style("✓").green().bold(),
        style(chunks.len()).cyan().bold(),
        style(by_file.len()).cyan().bold()
    );
    println!(
        "   Functions: {}   Classes: {}",
        style(funcs).white().bold(),
        style(classes).white().bold()
    );
    println!();

    // Top files
    let mut files_sorted: Vec<_> = by_file.iter().collect();
    files_sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    println!("   {}", style("Top files:").dim());
    for (file, file_chunks) in files_sorted.iter().take(10) {
        println!("   {:>4} chunks  {}", style(file_chunks.len()).cyan(), file);
    }
    if files_sorted.len() > 10 {
        println!("   {} more files...", style(files_sorted.len() - 10).dim());
    }

    Ok(())
}

fn cmd_analyse(
    repo_path: &str,
    config_path: &str,
    output_formats: &str,
    output_dir: &str,
    dry_run: bool,
) -> Result<()> {
    let mut config = load_config(config_path)?;
    config.output_dir = output_dir.to_string();
    config.outputs = output_formats
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let analyser = CoreAnalyser::new(config);

    // Step 1: Scan
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}")?);
    pb.set_message("Scanning repository...");
    pb.enable_steady_tick(Duration::from_millis(80));

    let chunks = analyser.scan(repo_path)?;
    pb.finish_with_message(format!(
        "{}  Found {} chunks",
        style("✓").green().bold(),
        style(chunks.len()).cyan().bold()
    ));

    // Step 2: Build requests
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}")?);
    pb.set_message("Building batch requests...");
    pb.enable_steady_tick(Duration::from_millis(80));

    let requests = analyser.build_requests(&chunks);
    let cost = estimate_cost(&requests);
    pb.finish_with_message(format!(
        "{}  {} requests built",
        style("✓").green().bold(),
        style(requests.len()).cyan().bold()
    ));

    // Cost summary
    println!("\n   Model:          {}", style(&cost.model).white().bold());
    println!(
        "   Input tokens:   ~{}",
        style(format_number(cost.estimated_input_tokens)).cyan()
    );
    println!(
        "   Output tokens:  ~{} (max)",
        style(format_number(cost.estimated_max_output_tokens)).cyan()
    );
    println!(
        "   Estimated cost: {}",
        style(format!("${:.4}", cost.estimated_cost_usd))
            .yellow()
            .bold()
    );

    if dry_run {
        println!(
            "\n   {} Dry run — no API call made.",
            style("ℹ").blue().bold()
        );

        // Write requests to file for manual submission
        let requests_path = format!("{}/batch-requests.jsonl", output_dir);
        std::fs::create_dir_all(output_dir)?;
        let mut jsonl = String::new();
        for req in &requests {
            jsonl.push_str(&serde_json::to_string(req)?);
            jsonl.push('\n');
        }
        std::fs::write(&requests_path, &jsonl)?;
        println!(
            "   Requests saved to: {}\n",
            style(&requests_path).underlined()
        );
        println!("   To submit manually:");
        println!("   1. Submit the JSONL to the Anthropic Batch API");
        println!("   2. Download the results JSONL");
        println!(
            "   3. Run: {} aggregate {} <results.jsonl>",
            style("astral").green(),
            repo_path
        );

        return Ok(());
    }

    // Step 3: Submit to Batch API
    let api_key = std::env::var("ANTHROPIC_API_KEY").context(
        "ANTHROPIC_API_KEY not set. Use --dry-run to generate requests without submitting.",
    )?;

    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}")?);
    pb.set_message("Submitting to Anthropic Batch API...");
    pb.enable_steady_tick(Duration::from_millis(80));

    let rt = tokio::runtime::Runtime::new()?;
    let batch_id = rt.block_on(submit_batch(&api_key, &requests))?;
    pb.finish_with_message(format!(
        "{}  Batch submitted: {}",
        style("✓").green().bold(),
        style(&batch_id).cyan()
    ));

    // Step 4: Poll
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}")?);
    pb.enable_steady_tick(Duration::from_millis(80));

    let raw_results = rt.block_on(poll_and_fetch(&api_key, &batch_id, &pb, requests.len()))?;
    pb.finish_with_message(format!("{}  Batch complete", style("✓").green().bold()));

    // Step 5: Aggregate
    let results = analyser.aggregate_results(&raw_results, &chunks)?;
    let stats = astral_core::aggregator::compute_stats(&results);
    println!(
        "\n   Results: {} succeeded, {} errored",
        style(stats.succeeded).green().bold(),
        style(stats.errored).red().bold()
    );

    // Step 6: Render outputs
    let written = outputs::write_all_outputs(&results, &analyser.config)?;
    println!();
    for path in &written {
        println!("   {} {}", style("→").dim(), style(path).underlined());
    }
    println!();

    Ok(())
}

fn cmd_aggregate(
    repo_path: &str,
    jsonl_path: &str,
    config_path: &str,
    output_formats: &str,
    output_dir: &str,
) -> Result<()> {
    let mut config = load_config(config_path)?;
    config.output_dir = output_dir.to_string();
    config.outputs = output_formats
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let analyser = CoreAnalyser::new(config);

    // Read JSONL
    let raw_results = std::fs::read_to_string(jsonl_path)
        .with_context(|| format!("Failed to read JSONL file: {}", jsonl_path))?;

    // Re-scan to get chunk mapping
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}")?);
    pb.set_message("Scanning repository for chunk mapping...");
    pb.enable_steady_tick(Duration::from_millis(80));

    let chunks = analyser.scan(repo_path)?;
    pb.finish_and_clear();

    // Aggregate
    let results = analyser.aggregate_results(&raw_results, &chunks)?;
    let stats = astral_core::aggregator::compute_stats(&results);

    println!(
        "\n{}  Aggregated {} results ({} succeeded, {} errored)",
        style("✓").green().bold(),
        style(stats.total).cyan().bold(),
        style(stats.succeeded).green(),
        style(stats.errored).red(),
    );

    // Render
    let written = outputs::write_all_outputs(&results, &analyser.config)?;
    println!();
    for path in &written {
        println!("   {} {}", style("→").dim(), style(path).underlined());
    }
    println!();

    Ok(())
}

// ─── Batch API helpers ───

async fn submit_batch(api_key: &str, requests: &[astral_core::BatchRequest]) -> Result<String> {
    let client = reqwest::Client::new();

    let resp = client
        .post("https://api.anthropic.com/v1/messages/batches")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "requests": requests.iter().map(|r| {
                serde_json::json!({
                    "custom_id": r.custom_id,
                    "params": {
                        "model": r.params.model,
                        "max_tokens": r.params.max_tokens,
                        "system": r.params.system,
                        "messages": r.params.messages,
                    }
                })
            }).collect::<Vec<_>>()
        }))
        .send()
        .await
        .context("Failed to submit batch")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("Batch API returned {}: {}", status, text);
    }

    let json: serde_json::Value = resp.json().await?;
    let batch_id = json["id"]
        .as_str()
        .context("No batch id in response")?
        .to_string();

    Ok(batch_id)
}

async fn poll_and_fetch(
    api_key: &str,
    batch_id: &str,
    pb: &ProgressBar,
    total: usize,
) -> Result<String> {
    let client = reqwest::Client::new();
    let url = format!("https://api.anthropic.com/v1/messages/batches/{}", batch_id);

    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;

        let resp = client
            .get(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .await
            .context("Failed to poll batch")?;

        let json: serde_json::Value = resp.json().await?;
        let status = json["processing_status"].as_str().unwrap_or("unknown");

        let succeeded = json["request_counts"]["succeeded"].as_u64().unwrap_or(0);
        let processing = json["request_counts"]["processing"].as_u64().unwrap_or(0);

        pb.set_message(format!(
            "Batch {}: {}/{} done ({} processing)",
            batch_id, succeeded, total, processing
        ));

        if status == "ended" {
            // Fetch results
            let results_url = format!("{}/results", url);
            let results_resp = client
                .get(&results_url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
                .context("Failed to fetch batch results")?;

            return Ok(results_resp.text().await?);
        }
    }
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
