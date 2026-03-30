//! Stats command: show usage analytics.

use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct StatsArgs {
    /// Analytics database path
    #[arg(short, long)]
    db: Option<PathBuf>,

    /// Time range (e.g., "1d", "7d", "30d", "all")
    #[arg(short, long, default_value = "7d")]
    range: String,

    /// Filter by skill
    #[arg(long)]
    skill: Option<String>,

    /// Output format
    #[arg(long, default_value = "text")]
    format: String,

    /// Show per-skill breakdown
    #[arg(long)]
    breakdown: bool,
}

pub async fn stats(args: StatsArgs) -> anyhow::Result<()> {
    // Note: This requires analytics to be collected
    // Placeholder implementation

    println!("Usage Statistics");
    println!("================");
    println!();

    if let Some(ref db) = args.db {
        println!("Database: {:?}", db);
    } else {
        println!("Database: (in-memory / not configured)");
    }

    println!("Time range: {}", args.range);

    if let Some(ref skill) = args.skill {
        println!("Filtered by skill: {}", skill);
    }

    println!();
    println!("Note: Analytics collection requires runtime integration.");
    println!("Use skm_learn::UsageAnalytics to record selection events.");

    Ok(())
}
