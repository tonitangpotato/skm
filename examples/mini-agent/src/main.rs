//! Mini Agent Example
//!
//! A minimal agent that demonstrates SKM trigger-based skill selection.
//! Loads skills from the ./skills directory and matches user queries.

use std::io::{self, BufRead, Write};

use skm_core::SkillRegistry;
use skm_select::{SelectionContext, SelectionStrategy, TriggerStrategy};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging (set RUST_LOG=debug for verbose output)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("mini_agent=info".parse().unwrap()),
        )
        .init();

    // Get the skills directory (relative to the example root)
    let skills_dir = std::env::current_dir()?.join("skills");

    if !skills_dir.exists() {
        eprintln!("Skills directory not found: {:?}", skills_dir);
        eprintln!("Run this example from examples/mini-agent/");
        std::process::exit(1);
    }

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    SKM Mini Agent Demo                       ║");
    println!("║                                                              ║");
    println!("║  Type a query to see which skills match.                     ║");
    println!("║  Type 'list' to see all available skills.                    ║");
    println!("║  Type 'quit' or Ctrl+C to exit.                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Load skills from the directory
    println!("Loading skills from {:?}...", skills_dir);
    let registry = SkillRegistry::new(&[&skills_dir]).await?;
    let skill_count = registry.len().await;
    println!("Loaded {} skills.\n", skill_count);

    // Build the trigger strategy from the registry
    let selector = TriggerStrategy::from_registry(&registry).await?;

    // Get the catalog for selection
    let catalog = registry.catalog().await;
    let ctx = SelectionContext::new();

    // Interactive loop
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("query> ");
        stdout.flush()?;

        let mut input = String::new();
        if stdin.lock().read_line(&mut input)? == 0 {
            break; // EOF
        }

        let query = input.trim();
        if query.is_empty() {
            continue;
        }

        // Handle special commands
        match query.to_lowercase().as_str() {
            "quit" | "exit" | "q" => {
                println!("Goodbye!");
                break;
            }
            "list" | "skills" | "ls" => {
                println!("\nAvailable skills:");
                println!("─────────────────────────────────────────────────────────");
                for meta in &catalog {
                    println!("  • {} - {}", meta.name, meta.description);
                    if !meta.triggers.is_empty() {
                        let triggers: Vec<_> = meta.triggers.iter().take(5).collect();
                        let trigger_str = triggers
                            .iter()
                            .map(|s| format!("\"{}\"", s))
                            .collect::<Vec<_>>()
                            .join(", ");
                        if meta.triggers.len() > 5 {
                            println!("    triggers: {} (+{} more)", trigger_str, meta.triggers.len() - 5);
                        } else {
                            println!("    triggers: {}", trigger_str);
                        }
                    }
                }
                println!();
                continue;
            }
            "help" | "?" => {
                println!("\nCommands:");
                println!("  list    - Show all available skills");
                println!("  quit    - Exit the program");
                println!("  <query> - Match skills against your query");
                println!();
                continue;
            }
            _ => {}
        }

        // Match the query against skills
        let refs: Vec<_> = catalog.iter().collect();
        let results = selector.select(query, &refs, &ctx).await?;

        if results.is_empty() {
            println!("  ❌ No skills matched.\n");
        } else {
            println!("  ✅ Matched {} skill(s):", results.len());
            println!("  ─────────────────────────────────────────────────────────");
            for result in &results {
                let confidence_emoji = match result.confidence {
                    skm_select::Confidence::Definite => "🎯",
                    skm_select::Confidence::High => "✓",
                    skm_select::Confidence::Medium => "~",
                    skm_select::Confidence::Low => "?",
                    skm_select::Confidence::None => "✗",
                };

                // Get the skill description from catalog
                let desc = catalog
                    .iter()
                    .find(|m| m.name == result.skill)
                    .map(|m| m.description.as_str())
                    .unwrap_or("");

                println!(
                    "  {} {} (score: {:.2}, confidence: {:?})",
                    confidence_emoji, result.skill, result.score, result.confidence
                );
                println!("    └─ {}", desc);
                if let Some(reasoning) = &result.reasoning {
                    println!("    └─ reason: {}", reasoning);
                }
            }
            println!();
        }
    }

    Ok(())
}
