//! Init command: create a new skill from template.

use clap::Args;
use std::path::PathBuf;

const SKILL_TEMPLATE: &str = r#"---
name: {{name}}
description: {{description}}
license: MIT
metadata:
  triggers: "{{triggers}}"
  tags: "{{tags}}"
---

# {{name}}

## Overview

{{description}}

## Usage

<!-- Add usage instructions here -->

## Examples

<!-- Add examples here -->
"#;

#[derive(Args)]
pub struct InitArgs {
    /// Skill name (will create a directory with this name)
    name: String,

    /// Skill description
    #[arg(short, long, default_value = "A new skill")]
    description: String,

    /// Trigger patterns (comma-separated)
    #[arg(short, long, default_value = "")]
    triggers: String,

    /// Tags (comma-separated)
    #[arg(long, default_value = "")]
    tags: String,

    /// Directory to create skill in
    #[arg(short, long, default_value = ".")]
    output: PathBuf,

    /// Force overwrite if exists
    #[arg(short, long)]
    force: bool,
}

pub async fn init(args: InitArgs) -> anyhow::Result<()> {
    let skill_dir = args.output.join(&args.name);

    if skill_dir.exists() && !args.force {
        anyhow::bail!(
            "Directory {:?} already exists. Use --force to overwrite.",
            skill_dir
        );
    }

    // Create directory
    std::fs::create_dir_all(&skill_dir)?;

    // Generate content
    let content = SKILL_TEMPLATE
        .replace("{{name}}", &args.name)
        .replace("{{description}}", &args.description)
        .replace("{{triggers}}", &args.triggers)
        .replace("{{tags}}", &args.tags);

    // Write SKILL.md
    let skill_file = skill_dir.join("SKILL.md");
    std::fs::write(&skill_file, content)?;

    println!("✓ Created skill at {:?}", skill_file);

    // Create optional directories
    std::fs::create_dir_all(skill_dir.join("references"))?;
    std::fs::create_dir_all(skill_dir.join("scripts"))?;

    println!("✓ Created references/ and scripts/ directories");
    println!("\nNext steps:");
    println!("  1. Edit {:?} to add instructions", skill_file);
    println!("  2. Run `skm validate {:?}` to check the skill", skill_dir);

    Ok(())
}
