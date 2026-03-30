//! Test command: run trigger test suites.

use clap::Args;
use std::path::PathBuf;

use skx_core::SkillRegistry;
use skx_learn::{TestSuite, TriggerTestHarness};
use skx_select::{CascadeSelector, TriggerStrategy};

#[derive(Args)]
pub struct TestArgs {
    /// Test suite file (YAML)
    suite: PathBuf,

    /// Skill directories
    #[arg(short, long, default_value = ".")]
    skills: Vec<PathBuf>,

    /// Verbose output (show each test)
    #[arg(short, long)]
    verbose: bool,

    /// Output format
    #[arg(long, default_value = "text")]
    format: String,
}

pub async fn test(args: TestArgs) -> anyhow::Result<()> {
    // Load test suite
    let suite = TestSuite::from_file(&args.suite)?;
    println!("Loaded test suite: {} ({} cases)", suite.name, suite.cases.len());

    // Load skills
    let registry = SkillRegistry::new(&args.skills).await?;
    let trigger = TriggerStrategy::from_registry(&registry).await?;
    let selector = CascadeSelector::builder().with_triggers(trigger).build();

    // Run tests
    let harness = TriggerTestHarness::new();
    let report = harness.run(&suite, &selector, &registry).await?;

    match args.format.as_str() {
        "json" => {
            let output = serde_json::json!({
                "suite": report.suite_name,
                "total": report.total,
                "passed": report.passed,
                "accuracy": report.accuracy(),
                "avg_latency_ms": report.avg_latency_ms,
                "results": report.results.iter().map(|r| {
                    serde_json::json!({
                        "name": r.name,
                        "passed": r.passed,
                        "selected": r.selected.as_ref().map(|s| s.as_str()),
                        "score": r.score,
                        "latency_ms": r.latency_ms
                    })
                }).collect::<Vec<_>>()
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        _ => {
            if args.verbose {
                println!("\nResults:");
                for r in &report.results {
                    let status = if r.passed { "✓" } else { "✗" };
                    let selected = r
                        .selected
                        .as_ref()
                        .map(|s| s.as_str())
                        .unwrap_or("(none)");
                    println!(
                        "  {} {} -> {} ({}ms)",
                        status, r.name, selected, r.latency_ms
                    );
                }
                println!();
            }

            // Summary
            println!("Test Results: {}/{} passed ({:.1}%)",
                report.passed,
                report.total,
                report.accuracy() * 100.0
            );
            println!("Average latency: {:.1}ms", report.avg_latency_ms);

            // Per-skill breakdown
            if !report.per_skill.is_empty() {
                println!("\nPer-skill metrics:");
                for (skill, metrics) in &report.per_skill {
                    println!(
                        "  {}: P={:.1}% R={:.1}% F1={:.1}%",
                        skill,
                        metrics.precision() * 100.0,
                        metrics.recall() * 100.0,
                        metrics.f1() * 100.0
                    );
                }
            }
        }
    }

    // Exit with error if tests failed
    if report.passed < report.total {
        std::process::exit(1);
    }

    Ok(())
}
