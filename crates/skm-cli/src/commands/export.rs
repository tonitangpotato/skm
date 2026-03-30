//! Export command: export metrics in various formats.

use clap::Args;
use std::path::PathBuf;

use skm_learn::SelectionMetrics;

#[derive(Args)]
pub struct ExportArgs {
    /// Export format (prometheus, json)
    #[arg(short, long, default_value = "prometheus")]
    format: String,

    /// Output file (defaults to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

pub async fn export(args: ExportArgs) -> anyhow::Result<()> {
    // Create empty metrics for demo
    // In production, this would load from a running instance or database
    let metrics = SelectionMetrics::new();

    let output = match args.format.as_str() {
        "prometheus" => metrics.to_prometheus(),
        "json" => {
            let summary = metrics.summary();
            serde_json::to_string_pretty(&serde_json::json!({
                "total_selections": summary.total_selections,
                "total_timeouts": summary.total_timeouts,
                "latency_p50_ms": summary.latency_p50,
                "latency_p95_ms": summary.latency_p95,
                "latency_p99_ms": summary.latency_p99,
                "by_confidence": summary.by_confidence,
                "by_strategy": summary.by_strategy
            }))?
        }
        other => {
            anyhow::bail!("Unknown format: {}. Use 'prometheus' or 'json'", other);
        }
    };

    if let Some(path) = args.output {
        std::fs::write(&path, &output)?;
        println!("Exported to {:?}", path);
    } else {
        println!("{}", output);
    }

    Ok(())
}
