//! Serve command: HTTP API server.

use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct ServeArgs {
    /// Host to bind to
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind to
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// Skill directories
    #[arg(short, long, default_value = ".")]
    skills: Vec<PathBuf>,

    /// Embedding index file
    #[arg(short, long)]
    index: Option<PathBuf>,

    /// Enable CORS
    #[arg(long)]
    cors: bool,
}

pub async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    use axum::{routing::get, routing::post, Router};
    use std::sync::Arc;
    
    use ase_core::SkillRegistry;
    use ase_select::{CascadeSelector, TriggerStrategy};

    println!("Starting ASE HTTP server...");
    println!("  Host: {}", args.host);
    println!("  Port: {}", args.port);
    println!("  Skills: {:?}", args.skills);

    // Load registry
    let registry = Arc::new(SkillRegistry::new(&args.skills).await?);
    println!("Loaded {} skills", registry.len().await);

    // Build selector
    let trigger = TriggerStrategy::from_registry(&registry).await?;
    let selector = Arc::new(CascadeSelector::builder().with_triggers(trigger).build());

    // Build router
    let app = Router::new()
        .route("/health", get(health))
        .route("/skills", get({
            let reg = Arc::clone(&registry);
            move || list_skills(reg)
        }))
        .route("/select", post({
            let sel = Arc::clone(&selector);
            let reg = Arc::clone(&registry);
            move |body| select_skill(sel, reg, body)
        }));

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "OK"
}

async fn list_skills(registry: Arc<ase_core::SkillRegistry>) -> axum::Json<serde_json::Value> {
    let catalog = registry.catalog().await;
    let skills: Vec<serde_json::Value> = catalog
        .iter()
        .map(|s| serde_json::json!({
            "name": s.name.as_str(),
            "description": s.description,
            "triggers": s.triggers,
            "tags": s.tags
        }))
        .collect();
    axum::Json(serde_json::json!({ "skills": skills }))
}

async fn select_skill(
    selector: Arc<ase_select::CascadeSelector>,
    registry: Arc<ase_core::SkillRegistry>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> axum::Json<serde_json::Value> {
    let query = body.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let ctx = ase_select::SelectionContext::new();

    match selector.select(query, &registry, &ctx).await {
        Ok(outcome) => {
            let results: Vec<serde_json::Value> = outcome
                .selected
                .iter()
                .take(5)
                .map(|r| serde_json::json!({
                    "skill": r.skill.as_str(),
                    "score": r.score,
                    "confidence": format!("{:?}", r.confidence),
                    "strategy": r.strategy
                }))
                .collect();
            axum::Json(serde_json::json!({
                "success": true,
                "results": results,
                "latency_ms": outcome.total_latency.as_millis()
            }))
        }
        Err(e) => axum::Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}
