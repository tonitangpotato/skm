//! Validate command: check SKILL.md files.

use clap::Args;
use std::path::PathBuf;

use skm_core::SkillParser;

#[derive(Args)]
pub struct ValidateArgs {
    /// Paths to validate (directories or SKILL.md files)
    paths: Vec<PathBuf>,

    /// Strict mode (fail on warnings)
    #[arg(long)]
    strict: bool,

    /// Output format
    #[arg(long, default_value = "text")]
    format: String,
}

pub async fn validate(args: ValidateArgs) -> anyhow::Result<()> {
    let parser = if args.strict {
        SkillParser::strict()
    } else {
        SkillParser::new()
    };

    let mut total = 0;
    let mut passed = 0;
    let mut errors = Vec::new();

    for path in &args.paths {
        let skill_files = find_skill_files(path);

        for file in skill_files {
            total += 1;

            match parser.parse_file(&file) {
                Ok(skill) => {
                    passed += 1;
                    if args.format == "text" {
                        println!("✓ {}: {} ({})", file.display(), skill.name, skill.description);
                    }
                }
                Err(e) => {
                    errors.push((file.clone(), e.to_string()));
                    if args.format == "text" {
                        eprintln!("✗ {}: {}", file.display(), e);
                    }
                }
            }
        }
    }

    if args.format == "json" {
        let result = serde_json::json!({
            "total": total,
            "passed": passed,
            "failed": total - passed,
            "errors": errors.iter().map(|(p, e)| {
                serde_json::json!({
                    "file": p.display().to_string(),
                    "error": e
                })
            }).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("\nValidation complete: {}/{} passed", passed, total);
    }

    if passed < total {
        std::process::exit(1);
    }

    Ok(())
}

fn find_skill_files(path: &PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if path.is_file() && path.file_name().map(|n| n == "SKILL.md").unwrap_or(false) {
        files.push(path.clone());
    } else if path.is_dir() {
        // Recursively find SKILL.md files
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    files.extend(find_skill_files(&entry_path));
                } else if entry_path.file_name().map(|n| n == "SKILL.md").unwrap_or(false) {
                    files.push(entry_path);
                }
            }
        }
    }

    files
}
