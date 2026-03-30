//! Index command: build/rebuild embedding index.

use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct IndexArgs {
    /// Skill directories
    #[arg(short, long, default_value = ".")]
    skills: Vec<PathBuf>,

    /// Output index file
    #[arg(short, long, default_value = ".skm-index.bin")]
    output: PathBuf,

    /// Embedding model (bge-m3, minilm)
    #[arg(short, long, default_value = "minilm")]
    model: String,

    /// Force rebuild even if cache is valid
    #[arg(short, long)]
    force: bool,
}

pub async fn index(args: IndexArgs) -> anyhow::Result<()> {
    println!("Building embedding index...");
    println!("  Skills: {:?}", args.skills);
    println!("  Output: {:?}", args.output);
    println!("  Model: {}", args.model);
    println!("  Force: {}", args.force);
    println!();

    // Check if we have embedding features
    #[cfg(not(any(feature = "embed-bge-m3", feature = "embed-minilm")))]
    {
        println!("Warning: No embedding providers compiled in.");
        println!("Rebuild with --features embed-bge-m3 or --features embed-minilm");
        return Ok(());
    }

    #[cfg(any(feature = "embed-bge-m3", feature = "embed-minilm"))]
    {
        use skm_core::SkillRegistry;
        use skm_embed::{ComponentWeights, EmbeddingIndex, EmbeddingProvider};

        let registry = SkillRegistry::new(&args.skills).await?;
        println!("Found {} skills", registry.len().await);

        // Create provider based on model
        let provider: Box<dyn EmbeddingProvider> = match args.model.as_str() {
            #[cfg(feature = "embed-bge-m3")]
            "bge-m3" | "bge" => {
                println!("Loading BGE-M3 model...");
                Box::new(skm_embed::BgeM3Provider::new()?)
            }
            #[cfg(feature = "embed-minilm")]
            "minilm" | "mini" => {
                println!("Loading MiniLM model...");
                Box::new(skm_embed::MiniLmProvider::new()?)
            }
            other => {
                anyhow::bail!("Unknown model: {}. Use 'bge-m3' or 'minilm'", other);
            }
        };

        println!("Building index (this may take a while)...");
        let index = EmbeddingIndex::build(
            &registry,
            provider.as_ref(),
            ComponentWeights::default(),
        )
        .await?;

        println!("Saving index to {:?}...", args.output);
        index.save(&args.output)?;

        println!("✓ Index built successfully ({} skills)", index.len());
    }

    Ok(())
}
