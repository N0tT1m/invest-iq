//! Symbol Search API Routes
//!
//! Endpoints for searching tickers and getting symbol details.

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{ApiResponse, AppError, AppState};

#[derive(Deserialize, utoipa::IntoParams)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SymbolSearchResult {
    pub ticker: String,
    pub name: String,
    pub exchange: String,
    pub symbol_type: String,
    pub currency: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SymbolDetail {
    pub ticker: String,
    pub name: String,
    pub description: Option<String>,
    pub market_cap: Option<f64>,
    pub homepage_url: Option<String>,
    pub sic_description: Option<String>,
    pub total_employees: Option<i64>,
    pub list_date: Option<String>,
    pub current_price: Option<f64>,
    pub change_percent: Option<f64>,
}

pub fn symbol_routes() -> Router<AppState> {
    Router::new()
        .route("/api/symbols/search", get(search_symbols))
        .route("/api/symbols/:symbol", get(get_symbol_detail))
}

#[utoipa::path(
    get,
    path = "/api/symbols/search",
    params(SearchQuery),
    responses((status = 200, description = "Matching ticker symbols")),
    tag = "Screener"
)]
async fn search_symbols(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<ApiResponse<Vec<SymbolSearchResult>>>, AppError> {
    let q = query.q.trim();
    if q.is_empty() {
        return Ok(Json(ApiResponse::success(Vec::new())));
    }

    let limit = query.limit.unwrap_or(20).min(50);

    let results = state
        .orchestrator
        .polygon_client
        .search_tickers(q, limit)
        .await
        .map_err(|e| anyhow::anyhow!("Search failed: {}", e))?;

    let symbols: Vec<SymbolSearchResult> = results
        .into_iter()
        .map(|r| SymbolSearchResult {
            ticker: r.ticker,
            name: r.name,
            exchange: r.primary_exchange,
            symbol_type: r.r#type,
            currency: r.currency_name,
        })
        .collect();

    Ok(Json(ApiResponse::success(symbols)))
}

#[utoipa::path(
    get,
    path = "/api/symbols/{symbol}",
    params(("symbol" = String, Path, description = "Stock ticker symbol")),
    responses((status = 200, description = "Detailed symbol information with current price")),
    tag = "Screener"
)]
async fn get_symbol_detail(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<SymbolDetail>>, AppError> {
    let symbol = symbol.to_uppercase();

    // Fetch ticker details and snapshot in parallel
    let (details_result, snapshot_result) = tokio::join!(
        state
            .orchestrator
            .polygon_client
            .get_ticker_details(&symbol),
        state.orchestrator.polygon_client.get_snapshot(&symbol),
    );

    let details =
        details_result.map_err(|e| anyhow::anyhow!("Failed to get ticker details: {}", e))?;

    let (current_price, change_percent) = match snapshot_result {
        Ok(snap) => {
            let price = snap.day.as_ref().and_then(|d| d.c);
            let change = snap.todays_change_perc;
            (price, change)
        }
        Err(_) => (None, None),
    };

    Ok(Json(ApiResponse::success(SymbolDetail {
        ticker: details.ticker.clone(),
        name: details.name.clone(),
        description: details.description.clone(),
        market_cap: details.market_cap,
        homepage_url: details.homepage_url.clone(),
        sic_description: details.sic_description.clone(),
        total_employees: details.total_employees,
        list_date: details.list_date.clone(),
        current_price,
        change_percent,
    })))
}
