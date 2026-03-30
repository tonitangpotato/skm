//! List command: list skills in a directory.

use clap::Args;
use std::path::PathBuf;

use ase_core::SkillRegistry;

#[derive(Args)]
pub struct ListArgs {
    /// Skill directories to scan
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,

    /// Output format (text, json, csv)
    #[arg(short, long, default_value = "text")]
    format: String,

    /// Show triggers
    #[arg(long)]
    triggers: bool,

    /// Show tags
    #[arg(long)]
    tags: bool,

    /// Show token estimates
    #[arg(long)]
    tokens: bool,
}

pub async fn list(args: ListArgs) -> anyhow::Result<()> {
    let registry = SkillRegistry::new(&args.paths).await?;
    let catalog = registry.catalog().await;

    match args.format.as_str() {
        "json" => {
            let skills: Vec<serde_json::Value> = catalog
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "name": s.name.as_str(),
                        "description": s.description,
                        "triggers": s.triggers,
                        "tags": s.tags,
                        "estimated_tokens": s.estimated_tokens,
                        "source": s.source_path.display().to_string()
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&skills)?);
        }
        "csv" => {
            println!("name,description,triggers,tags,tokens");
            for s in &catalog {
                println!(
                    "{},{:?},{:?},{:?},{}",
                    s.name,
                    s.description,
                    s.triggers.join(";"),
                    s.tags.join(";"),
                    s.estimated_tokens
                );
            }
        }
        _ => {
            println!("Found {} skills:\n", catalog.len());

            for s in &catalog {
                println!("  {} - {}", s.name, s.description);

                if args.triggers && !s.triggers.is_empty() {
                    println!("    triggers: {}", s.triggers.join(", "));
                }

                if args.tags && !s.tags.is_empty() {
                    println!("    tags: {}", s.tags.join(", "));
                }

                if args.tokens {
                    println!("    tokens: ~{}", s.estimated_tokens);
                }
            }
        }
    }

    Ok(())
}
