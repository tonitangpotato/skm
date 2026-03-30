//! Select command: interactive skill selection for debugging.

use clap::Args;
use std::path::PathBuf;

use skx_core::SkillRegistry;
use skx_select::{CascadeSelector, SelectionContext, TriggerStrategy};

#[derive(Args)]
pub struct SelectArgs {
    /// Query to select skill for
    query: String,

    /// Skill directories
    #[arg(short, long, default_value = ".")]
    skills: Vec<PathBuf>,

    /// Top K results to return
    #[arg(short = 'k', long, default_value = "5")]
    top_k: usize,

    /// Show detailed breakdown
    #[arg(long)]
    verbose: bool,

    /// Output format
    #[arg(long, default_value = "text")]
    format: String,
}

pub async fn select(args: SelectArgs) -> anyhow::Result<()> {
    let registry = SkillRegistry::new(&args.skills).await?;

    // Build trigger strategy
    let trigger = TriggerStrategy::from_registry(&registry).await?;

    // Build cascade
    let selector = CascadeSelector::builder().with_triggers(trigger).build();

    // Select
    let ctx = SelectionContext::new();
    let outcome = selector.select(&args.query, &registry, &ctx).await?;

    match args.format.as_str() {
        "json" => {
            let results: Vec<serde_json::Value> = outcome
                .selected
                .iter()
                .take(args.top_k)
                .map(|r| {
                    serde_json::json!({
                        "skill": r.skill.as_str(),
                        "score": r.score,
                        "confidence": format!("{:?}", r.confidence),
                        "strategy": r.strategy,
                        "reasoning": r.reasoning
                    })
                })
                .collect();

            let output = serde_json::json!({
                "query": args.query,
                "results": results,
                "latency_ms": outcome.total_latency.as_millis(),
                "strategies_used": outcome.strategies_used,
                "fallback_used": outcome.fallback_used
            });

            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        _ => {
            println!("Query: {}\n", args.query);

            if outcome.selected.is_empty() {
                println!("No matching skills found.");
            } else {
                println!("Results:");
                for (i, r) in outcome.selected.iter().take(args.top_k).enumerate() {
                    println!(
                        "  {}. {} (score: {:.2}, confidence: {:?}, strategy: {})",
                        i + 1,
                        r.skill,
                        r.score,
                        r.confidence,
                        r.strategy
                    );
                    if args.verbose {
                        if let Some(ref reasoning) = r.reasoning {
                            println!("     reasoning: {}", reasoning);
                        }
                    }
                }
            }

            println!("\nLatency: {:?}", outcome.total_latency);
            println!("Strategies: {:?}", outcome.strategies_used);
        }
    }

    Ok(())
}
