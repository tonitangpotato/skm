use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};

mod commands;

#[cfg(feature = "http-server")]
mod server;

/// Agent Skill Engine CLI
#[derive(Parser)]
#[command(name = "skx")]
#[command(about = "Agent Skill Engine - Selection, enforcement, and optimization for SKILL.md")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new skill from template
    Init(commands::InitArgs),

    /// Validate SKILL.md files
    Validate(commands::ValidateArgs),

    /// List skills in a directory
    List(commands::ListArgs),

    /// Interactive skill selection (for debugging)
    Select(commands::SelectArgs),

    /// Run trigger test suites
    Test(commands::TestArgs),

    /// Benchmark selection performance
    Bench(commands::BenchArgs),

    /// Optimize skill descriptions
    Optimize(commands::OptimizeArgs),

    /// Show usage analytics
    Stats(commands::StatsArgs),

    /// Build/rebuild embedding index
    Index(commands::IndexArgs),

    /// Export metrics (Prometheus/JSON)
    Export(commands::ExportArgs),

    /// Start HTTP API server
    #[cfg(feature = "http-server")]
    Serve(commands::ServeArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    fmt().with_env_filter(filter).init();

    match cli.command {
        Commands::Init(args) => commands::init(args).await,
        Commands::Validate(args) => commands::validate(args).await,
        Commands::List(args) => commands::list(args).await,
        Commands::Select(args) => commands::select(args).await,
        Commands::Test(args) => commands::test(args).await,
        Commands::Bench(args) => commands::bench(args).await,
        Commands::Optimize(args) => commands::optimize(args).await,
        Commands::Stats(args) => commands::stats(args).await,
        Commands::Index(args) => commands::index(args).await,
        Commands::Export(args) => commands::export(args).await,
        #[cfg(feature = "http-server")]
        Commands::Serve(args) => commands::serve(args).await,
    }
}
