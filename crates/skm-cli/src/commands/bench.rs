//! Bench command: benchmark selection performance.

use clap::Args;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use skm_core::SkillRegistry;
use skm_select::{CascadeSelector, SelectionContext, TriggerStrategy};

#[derive(Args)]
pub struct BenchArgs {
    /// Queries to benchmark (one per line in file, or comma-separated)
    queries: Vec<String>,

    /// File containing queries (one per line)
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Skill directories
    #[arg(short, long, default_value = ".")]
    skills: Vec<PathBuf>,

    /// Number of iterations
    #[arg(short, long, default_value = "100")]
    iterations: usize,

    /// Warmup iterations
    #[arg(long, default_value = "10")]
    warmup: usize,

    /// Output format
    #[arg(long, default_value = "text")]
    format: String,
}

pub async fn bench(args: BenchArgs) -> anyhow::Result<()> {
    // Collect queries
    let mut queries = args.queries.clone();
    if let Some(ref file) = args.file {
        let content = std::fs::read_to_string(file)?;
        queries.extend(content.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()));
    }

    if queries.is_empty() {
        anyhow::bail!("No queries provided. Use positional args or --file.");
    }

    // Load skills
    let registry = SkillRegistry::new(&args.skills).await?;
    let trigger = TriggerStrategy::from_registry(&registry).await?;
    let selector = CascadeSelector::builder().with_triggers(trigger).build();
    let ctx = SelectionContext::new();

    // Warmup
    println!("Warming up ({} iterations)...", args.warmup);
    for _ in 0..args.warmup {
        for query in &queries {
            let _ = selector.select(query, &registry, &ctx).await;
        }
    }

    // Benchmark
    println!("Benchmarking ({} iterations)...", args.iterations);
    let mut latencies = Vec::with_capacity(args.iterations * queries.len());

    let bench_start = Instant::now();
    for _ in 0..args.iterations {
        for query in &queries {
            let start = Instant::now();
            let _ = selector.select(query, &registry, &ctx).await;
            latencies.push(start.elapsed());
        }
    }
    let total_time = bench_start.elapsed();

    // Calculate stats
    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[(latencies.len() as f64 * 0.95) as usize];
    let p99 = latencies[(latencies.len() as f64 * 0.99) as usize];
    let min = latencies[0];
    let max = latencies[latencies.len() - 1];
    let avg: Duration = latencies.iter().sum::<Duration>() / latencies.len() as u32;
    let throughput = latencies.len() as f64 / total_time.as_secs_f64();

    match args.format.as_str() {
        "json" => {
            let output = serde_json::json!({
                "queries": queries.len(),
                "iterations": args.iterations,
                "total_selections": latencies.len(),
                "total_time_ms": total_time.as_millis(),
                "throughput_per_sec": throughput,
                "latency": {
                    "min_us": min.as_micros(),
                    "max_us": max.as_micros(),
                    "avg_us": avg.as_micros(),
                    "p50_us": p50.as_micros(),
                    "p95_us": p95.as_micros(),
                    "p99_us": p99.as_micros()
                }
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        _ => {
            println!("\nBenchmark Results:");
            println!("  Queries: {}", queries.len());
            println!("  Iterations: {}", args.iterations);
            println!("  Total selections: {}", latencies.len());
            println!("  Total time: {:?}", total_time);
            println!("  Throughput: {:.1} selections/sec", throughput);
            println!("\nLatency:");
            println!("  Min: {:?}", min);
            println!("  Max: {:?}", max);
            println!("  Avg: {:?}", avg);
            println!("  P50: {:?}", p50);
            println!("  P95: {:?}", p95);
            println!("  P99: {:?}", p99);
        }
    }

    Ok(())
}
