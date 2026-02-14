use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use portfolio_manager::*;
use rust_decimal::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::{get_cached_etf_bars, ApiResponse, AppError, AppState};

#[derive(Deserialize)]
pub struct DaysQuery {
    pub days: Option<i64>,
}

#[derive(Deserialize)]
pub struct BenchmarkQuery {
    pub symbol: Option<String>,
    pub days: Option<i64>,
}

#[derive(Deserialize)]
pub struct PerformanceMethodQuery {
    pub days: Option<i64>,
    pub method: Option<String>,
}

#[derive(Deserialize)]
pub struct TaxSummaryQuery {
    pub jurisdiction: Option<String>,
}

#[derive(Deserialize)]
pub struct TaxImpactRequest {
    pub symbol: String,
    pub shares: f64,
    pub price: f64,
    pub jurisdiction: Option<String>,
}

#[derive(Deserialize)]
pub struct WashSaleQuery {
    pub symbol: String,
}

#[derive(Deserialize)]
pub struct ReconcileRequest {
    pub auto_resolve: Option<bool>,
}

#[derive(Deserialize)]
pub struct CsvImportRequest {
    pub csv_data: String,
}

#[derive(Deserialize)]
pub struct AllocationRequest {
    pub symbol: Option<String>,
    pub sector: Option<String>,
    pub target_weight_percent: f64,
    pub drift_tolerance_percent: Option<f64>,
}

pub fn portfolio_analytics_routes() -> Router<AppState> {
    Router::new()
        // Risk & Performance
        .route("/api/portfolio/risk-metrics", get(get_risk_metrics))
        .route(
            "/api/portfolio/performance-analytics",
            get(get_performance_analytics),
        )
        .route("/api/portfolio/benchmark", get(get_benchmark))
        // Reconciliation
        .route("/api/portfolio/reconcile", post(reconcile_positions))
        .route(
            "/api/portfolio/reconciliation-log",
            get(get_reconciliation_log),
        )
        // Allocations & Rebalancing
        .route(
            "/api/portfolio/allocations",
            get(get_allocations).post(set_allocation),
        )
        .route("/api/portfolio/allocations/:id", delete(delete_allocation))
        .route("/api/portfolio/rebalance", get(get_rebalance_proposal))
        .route("/api/portfolio/drift", get(get_drift))
        // Tax
        .route("/api/portfolio/tax-summary", get(get_tax_summary))
        .route("/api/portfolio/tax-impact", post(estimate_tax_impact))
        .route("/api/portfolio/wash-sale-check", get(check_wash_sale))
        // Enhanced trades
        .route(
            "/api/trades/enhanced-performance",
            get(get_enhanced_performance),
        )
        .route("/api/trades/import-csv", post(import_csv_trades))
        // Alert accuracy
        .route("/api/alerts/accuracy", get(get_alert_accuracy))
        .route("/api/alerts/:id/executions", get(get_alert_executions))
}

// ============================================================
// Helpers
// ============================================================

fn get_portfolio_manager(state: &AppState) -> Result<&PortfolioManager, AppError> {
    state
        .portfolio_manager
        .as_ref()
        .map(|pm| pm.as_ref())
        .ok_or_else(|| anyhow::anyhow!("Portfolio manager not initialized").into())
}

fn get_trade_logger(state: &AppState) -> Result<&TradeLogger, AppError> {
    state
        .trade_logger
        .as_ref()
        .map(|tl| tl.as_ref())
        .ok_or_else(|| anyhow::anyhow!("Trade logger not initialized").into())
}

fn get_alert_manager(state: &AppState) -> Result<&AlertManager, AppError> {
    state
        .alert_manager
        .as_ref()
        .map(|am| am.as_ref())
        .ok_or_else(|| anyhow::anyhow!("Alert manager not initialized").into())
}

/// Build positions with P&L by fetching current prices.
async fn build_positions_with_pnl(state: &AppState) -> Result<Vec<PositionWithPnL>, AppError> {
    let pm = get_portfolio_manager(state)?;
    let orchestrator = state.orchestrator.clone();
    let price_fetcher = move |symbol: &str| -> anyhow::Result<f64> {
        let rt = tokio::runtime::Handle::current();
        let symbol = symbol.to_string();
        let orch = orchestrator.clone();
        tokio::task::block_in_place(|| {
            rt.block_on(async move {
                let bars = orch
                    .get_bars(&symbol, analysis_core::Timeframe::Day1, 1)
                    .await?;
                bars.last()
                    .map(|bar| bar.close)
                    .ok_or_else(|| anyhow::anyhow!("No price data for {}", symbol))
            })
        })
    };
    let summary = pm.get_portfolio_summary(price_fetcher).await?;
    Ok(summary.positions)
}

fn parse_jurisdiction(s: &str) -> tax_optimizer::TaxJurisdiction {
    match s.to_uppercase().as_str() {
        "US" | "USA" => tax_optimizer::TaxJurisdiction::US,
        "UK" | "GB" => tax_optimizer::TaxJurisdiction::UK,
        "CA" | "CANADA" => tax_optimizer::TaxJurisdiction::Canada,
        "AU" | "AUSTRALIA" => tax_optimizer::TaxJurisdiction::Australia,
        "DE" | "GERMANY" => tax_optimizer::TaxJurisdiction::Germany,
        _ => tax_optimizer::TaxJurisdiction::US,
    }
}

// ============================================================
// Feature 1: Risk Metrics
// ============================================================

async fn get_risk_metrics(
    State(state): State<AppState>,
    Query(query): Query<DaysQuery>,
) -> Result<Json<ApiResponse<PortfolioRiskMetrics>>, AppError> {
    let pm = get_portfolio_manager(&state)?;
    let days = query.days.unwrap_or(90);
    let snapshots = pm.get_snapshots(days).await?;
    let positions = build_positions_with_pnl(&state).await?;

    // Build sector map from ticker details if available (otherwise empty)
    let sector_map: HashMap<String, String> = HashMap::new();

    let metrics = RiskCalculator::compute(&snapshots, &positions, &sector_map);
    Ok(Json(ApiResponse::success(metrics)))
}

// ============================================================
// Feature 2: Performance Analytics
// ============================================================

async fn get_performance_analytics(
    State(state): State<AppState>,
    Query(query): Query<DaysQuery>,
) -> Result<Json<ApiResponse<PerformanceAnalytics>>, AppError> {
    let pm = get_portfolio_manager(&state)?;
    let tl = get_trade_logger(&state)?;
    let days = query.days.unwrap_or(365);
    let snapshots = pm.get_snapshots(days).await?;
    let positions = build_positions_with_pnl(&state).await?;
    let trades = tl.get_all_trades(None).await?;

    let analytics = RiskCalculator::compute_performance(&snapshots, &positions, &trades);
    Ok(Json(ApiResponse::success(analytics)))
}

// ============================================================
// Feature 8: Benchmark
// ============================================================

async fn get_benchmark(
    State(state): State<AppState>,
    Query(query): Query<BenchmarkQuery>,
) -> Result<Json<ApiResponse<BenchmarkAnalysis>>, AppError> {
    let pm = get_portfolio_manager(&state)?;
    let benchmark_symbol = query.symbol.as_deref().unwrap_or("SPY");
    let days = query.days.unwrap_or(365);
    let snapshots = pm.get_snapshots(days).await?;

    // Fetch benchmark bars from ETF cache
    let bars = get_cached_etf_bars(&state, benchmark_symbol, days, 15).await;

    let benchmark_prices: Vec<(String, f64)> = bars
        .iter()
        .map(|b| (b.timestamp.format("%Y-%m-%d").to_string(), b.close))
        .collect();

    let analysis = BenchmarkComparer::compare(&snapshots, &benchmark_prices, benchmark_symbol);
    Ok(Json(ApiResponse::success(analysis)))
}

// ============================================================
// Feature 5: Reconciliation
// ============================================================

async fn reconcile_positions(
    State(state): State<AppState>,
    Json(req): Json<ReconcileRequest>,
) -> Result<Json<ApiResponse<ReconciliationResult>>, AppError> {
    let pm = get_portfolio_manager(&state)?;
    let broker = state
        .broker_client
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Broker not initialized"))?;

    // Fetch broker positions
    let broker_positions = broker.get_positions().await?;
    let broker: Vec<BrokerPosition> = broker_positions
        .iter()
        .map(|p| BrokerPosition {
            symbol: p.symbol.clone(),
            shares: p
                .qty
                .parse::<f64>()
                .ok()
                .and_then(rust_decimal::Decimal::from_f64)
                .unwrap_or_default(),
            avg_entry_price: p
                .avg_entry_price
                .parse::<f64>()
                .ok()
                .and_then(rust_decimal::Decimal::from_f64)
                .unwrap_or_default(),
            market_value: p
                .market_value
                .parse::<f64>()
                .ok()
                .and_then(rust_decimal::Decimal::from_f64)
                .unwrap_or_default(),
            current_price: p
                .current_price
                .parse::<f64>()
                .ok()
                .and_then(rust_decimal::Decimal::from_f64)
                .unwrap_or_default(),
            unrealized_pnl: p
                .unrealized_pl
                .parse::<f64>()
                .ok()
                .and_then(rust_decimal::Decimal::from_f64)
                .unwrap_or_default(),
        })
        .collect();

    let auto_resolve = req.auto_resolve.unwrap_or(false);
    let result = Reconciler::reconcile(pm, &broker, auto_resolve).await?;

    // Save log
    let _ = Reconciler::save_log(pm.db(), &result).await;

    Ok(Json(ApiResponse::success(result)))
}

async fn get_reconciliation_log(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ReconciliationLogEntry>>>, AppError> {
    let pm = get_portfolio_manager(&state)?;
    let entries = Reconciler::get_log(pm.db(), 50).await?;
    Ok(Json(ApiResponse::success(entries)))
}

// ============================================================
// Feature 6: Allocations & Rebalancing
// ============================================================

async fn get_allocations(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<TargetAllocation>>>, AppError> {
    let pm = get_portfolio_manager(&state)?;
    let allocations = pm.get_target_allocations().await?;
    Ok(Json(ApiResponse::success(allocations)))
}

async fn set_allocation(
    State(state): State<AppState>,
    Json(req): Json<AllocationRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let pm = get_portfolio_manager(&state)?;
    let target = TargetAllocation {
        id: None,
        symbol: req.symbol,
        sector: req.sector,
        target_weight_percent: req.target_weight_percent,
        drift_tolerance_percent: req.drift_tolerance_percent.unwrap_or(5.0),
        updated_at: None,
    };
    let id = pm.set_target_allocation(target).await?;
    Ok(Json(ApiResponse::success(serde_json::json!({ "id": id }))))
}

async fn delete_allocation(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let pm = get_portfolio_manager(&state)?;
    pm.delete_target_allocation(id).await?;
    Ok(Json(ApiResponse::success(
        serde_json::json!({ "message": "Allocation deleted" }),
    )))
}

async fn get_rebalance_proposal(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<RebalanceProposal>>, AppError> {
    let pm = get_portfolio_manager(&state)?;
    let positions = build_positions_with_pnl(&state).await?;
    let targets = pm.get_target_allocations().await?;

    let total_value: rust_decimal::Decimal = positions.iter().map(|p| p.market_value).sum();

    // Get current prices
    let prices: HashMap<String, f64> = positions
        .iter()
        .map(|p| {
            (
                p.position.symbol.clone(),
                p.current_price.to_f64().unwrap_or(0.0),
            )
        })
        .collect();

    let sector_map: HashMap<String, String> = HashMap::new();
    let proposal =
        RebalanceCalculator::calculate(&positions, &targets, total_value, &prices, &sector_map);
    Ok(Json(ApiResponse::success(proposal)))
}

async fn get_drift(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<DriftEntry>>>, AppError> {
    let pm = get_portfolio_manager(&state)?;
    let positions = build_positions_with_pnl(&state).await?;
    let targets = pm.get_target_allocations().await?;

    let total_value: rust_decimal::Decimal = positions.iter().map(|p| p.market_value).sum();

    let sector_map: HashMap<String, String> = HashMap::new();
    let drift = RebalanceCalculator::compute_drift(&positions, &targets, total_value, &sector_map);
    Ok(Json(ApiResponse::success(drift)))
}

// ============================================================
// Feature 7: Tax
// ============================================================

async fn get_tax_summary(
    State(state): State<AppState>,
    Query(query): Query<TaxSummaryQuery>,
) -> Result<Json<ApiResponse<TaxSummary>>, AppError> {
    let tl = get_trade_logger(&state)?;
    let trades = tl.get_all_trades(None).await?;

    let jurisdiction = parse_jurisdiction(query.jurisdiction.as_deref().unwrap_or("US"));

    // Get current prices for open lots
    let positions = build_positions_with_pnl(&state).await.unwrap_or_default();
    let prices: HashMap<String, f64> = positions
        .iter()
        .map(|p| {
            (
                p.position.symbol.clone(),
                p.current_price.to_f64().unwrap_or(0.0),
            )
        })
        .collect();

    let summary = TaxBridge::compute_tax_summary(&trades, &prices, jurisdiction);
    Ok(Json(ApiResponse::success(summary)))
}

async fn estimate_tax_impact(
    State(state): State<AppState>,
    Json(req): Json<TaxImpactRequest>,
) -> Result<Json<ApiResponse<TaxImpactEstimate>>, AppError> {
    let tl = get_trade_logger(&state)?;
    let trades = tl.get_all_trades(None).await?;
    let jurisdiction = parse_jurisdiction(req.jurisdiction.as_deref().unwrap_or("US"));

    let estimate = TaxBridge::estimate_tax_impact(
        &trades,
        &req.symbol.to_uppercase(),
        req.shares,
        req.price,
        jurisdiction,
    );
    Ok(Json(ApiResponse::success(estimate)))
}

async fn check_wash_sale(
    State(state): State<AppState>,
    Query(query): Query<WashSaleQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let tl = get_trade_logger(&state)?;
    let trades = tl.get_all_trades(None).await?;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let at_risk = TaxBridge::check_wash_sale_risk(&trades, &query.symbol.to_uppercase(), &today);
    Ok(Json(ApiResponse::success(serde_json::json!({
        "symbol": query.symbol.to_uppercase(),
        "wash_sale_risk": at_risk,
    }))))
}

// ============================================================
// Feature 3: Enhanced Trade Performance
// ============================================================

async fn get_enhanced_performance(
    State(state): State<AppState>,
    Query(query): Query<PerformanceMethodQuery>,
) -> Result<Json<ApiResponse<EnhancedPerformanceMetrics>>, AppError> {
    let tl = get_trade_logger(&state)?;
    let method = match query.method.as_deref() {
        Some("lifo") => CostBasisMethod::Lifo,
        Some("average") | Some("avg") => CostBasisMethod::AverageCost,
        _ => CostBasisMethod::Fifo,
    };
    let metrics = tl.get_enhanced_metrics(query.days, method).await?;
    Ok(Json(ApiResponse::success(metrics)))
}

// ============================================================
// Feature 5: CSV Import
// ============================================================

async fn import_csv_trades(
    State(state): State<AppState>,
    Json(req): Json<CsvImportRequest>,
) -> Result<Json<ApiResponse<ImportResult>>, AppError> {
    let tl = get_trade_logger(&state)?;
    let rows = Reconciler::parse_csv_trades(&req.csv_data)?;
    let result = Reconciler::import_csv_trades(tl, &rows).await?;
    Ok(Json(ApiResponse::success(result)))
}

// ============================================================
// Feature 4: Alert Accuracy
// ============================================================

async fn get_alert_accuracy(
    State(state): State<AppState>,
    Query(query): Query<DaysQuery>,
) -> Result<Json<ApiResponse<AlertAccuracyReport>>, AppError> {
    let am = get_alert_manager(&state)?;
    let report = am.get_accuracy_report(query.days).await?;
    Ok(Json(ApiResponse::success(report)))
}

async fn get_alert_executions(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<AlertExecution>>>, AppError> {
    let am = get_alert_manager(&state)?;
    let executions = am.get_executions_for_alert(id).await?;
    Ok(Json(ApiResponse::success(executions)))
}
