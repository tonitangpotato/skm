//! Serve command: HTTP API server.

use clap::Args;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use tower_http::cors::{Any, CorsLayer};

use skm_core::SkillRegistry;
use skm_learn::SelectionMetrics;
use skm_select::{CascadeSelector, SelectionContext, TriggerStrategy};

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

    /// Embedding index file (optional)
    #[arg(short, long)]
    index: Option<PathBuf>,

    /// Enable CORS for all origins
    #[arg(long)]
    cors: bool,
}

/// Shared application state
struct AppState {
    registry: SkillRegistry,
    selector: CascadeSelector,
    metrics: SelectionMetrics,
}

/// Request body for skill selection
#[derive(serde::Deserialize)]
struct SelectRequest {
    query: String,
    #[serde(default)]
    context: Option<serde_json::Value>,
}

/// Response for successful selection
#[derive(serde::Serialize)]
struct SelectResponse {
    success: bool,
    results: Vec<SkillResult>,
    latency_ms: u128,
    strategies_used: Vec<String>,
}

/// Individual skill result
#[derive(serde::Serialize)]
struct SkillResult {
    skill: String,
    score: f32,
    confidence: String,
    strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<String>,
}

/// Response for skill details
#[derive(serde::Serialize)]
struct SkillDetailResponse {
    name: String,
    description: String,
    triggers: Vec<String>,
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    source_path: String,
    estimated_tokens: usize,
}

/// Response for skill list
#[derive(serde::Serialize)]
struct SkillListResponse {
    skills: Vec<SkillSummary>,
    total: usize,
}

/// Summary of a skill for listing
#[derive(serde::Serialize)]
struct SkillSummary {
    name: String,
    description: String,
    triggers: Vec<String>,
    tags: Vec<String>,
}

/// Error response
#[derive(serde::Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}

pub async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    println!("Starting SKM HTTP server...");
    println!("  Host: {}", args.host);
    println!("  Port: {}", args.port);
    println!("  Skills: {:?}", args.skills);
    println!("  CORS: {}", if args.cors { "enabled" } else { "disabled" });

    // Load registry
    let registry = SkillRegistry::new(&args.skills).await?;
    println!("Loaded {} skills", registry.len().await);

    // Build selector
    let trigger = TriggerStrategy::from_registry(&registry).await?;
    let selector = CascadeSelector::builder().with_triggers(trigger).build();

    // Create metrics
    let metrics = SelectionMetrics::new();

    // Build shared state
    let state = Arc::new(AppState {
        registry,
        selector,
        metrics,
    });

    // Build router
    let mut app = Router::new()
        .route("/health", get(health))
        .route("/skills", get(list_skills))
        .route("/skills/{name}", get(get_skill))
        .route("/select", post(select_skill))
        .route("/metrics", get(get_metrics))
        .with_state(state);

    // Add CORS if enabled
    if args.cors {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
        app = app.layer(cors);
    }

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Listening on http://{}", addr);
    println!();
    println!("Endpoints:");
    println!("  GET  /health         - Health check");
    println!("  GET  /skills         - List all skills");
    println!("  GET  /skills/:name   - Get skill details");
    println!("  POST /select         - Select skills for a query");
    println!("  GET  /metrics        - Prometheus metrics");

    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

/// List all registered skills
async fn list_skills(State(state): State<Arc<AppState>>) -> Json<SkillListResponse> {
    let catalog = state.registry.catalog().await;
    let skills: Vec<SkillSummary> = catalog
        .iter()
        .map(|s| SkillSummary {
            name: s.name.as_str().to_string(),
            description: s.description.clone(),
            triggers: s.triggers.clone(),
            tags: s.tags.clone(),
        })
        .collect();
    let total = skills.len();
    Json(SkillListResponse { skills, total })
}

/// Get details for a specific skill
async fn get_skill(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<SkillDetailResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Find the skill in the catalog
    let catalog = state.registry.catalog().await;
    let skill_meta = catalog.iter().find(|s| s.name.as_str() == name);

    match skill_meta {
        Some(meta) => {
            // Try to get full skill (with instructions)
            let instructions = if let Ok(skill_name) = skm_core::SkillName::new(&name) {
                state
                    .registry
                    .get(&skill_name)
                    .await
                    .ok()
                    .map(|s| s.instructions.clone())
            } else {
                None
            };

            Ok(Json(SkillDetailResponse {
                name: meta.name.as_str().to_string(),
                description: meta.description.clone(),
                triggers: meta.triggers.clone(),
                tags: meta.tags.clone(),
                instructions,
                source_path: meta.source_path.display().to_string(),
                estimated_tokens: meta.estimated_tokens,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                error: format!("Skill '{}' not found", name),
            }),
        )),
    }
}

/// Select skills for a query
async fn select_skill(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SelectRequest>,
) -> Json<serde_json::Value> {
    let start = Instant::now();
    let ctx = SelectionContext::new();

    match state.selector.select(&body.query, &state.registry, &ctx).await {
        Ok(outcome) => {
            let latency = start.elapsed();

            // Record metrics for each selected skill
            for result in &outcome.selected {
                state.metrics.record_selection(
                    &result.skill,
                    &result.strategy,
                    result.confidence,
                    latency,
                );
            }

            // If no skills selected, record as no match
            if outcome.selected.is_empty() {
                state.metrics.record_no_match(latency);
            }

            let results: Vec<SkillResult> = outcome
                .selected
                .iter()
                .take(10)
                .map(|r| SkillResult {
                    skill: r.skill.as_str().to_string(),
                    score: r.score,
                    confidence: format!("{:?}", r.confidence),
                    strategy: r.strategy.clone(),
                    reasoning: r.reasoning.clone(),
                })
                .collect();

            Json(serde_json::json!({
                "success": true,
                "results": results,
                "latency_ms": outcome.total_latency.as_millis(),
                "strategies_used": outcome.strategies_used
            }))
        }
        Err(e) => {
            state.metrics.record_timeout(start.elapsed());
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            }))
        }
    }
}

/// Get Prometheus metrics
async fn get_metrics(State(state): State<Arc<AppState>>) -> String {
    state.metrics.to_prometheus()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_request_deserialize() {
        let json = r#"{"query": "test query"}"#;
        let req: SelectRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "test query");
        assert!(req.context.is_none());
    }

    #[test]
    fn test_select_request_with_context() {
        let json = r#"{"query": "test", "context": {"key": "value"}}"#;
        let req: SelectRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "test");
        assert!(req.context.is_some());
    }
}
