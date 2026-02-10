//! Tax Optimization API Routes
//!
//! Endpoints for tax-loss harvesting and wash sale monitoring.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tax_optimizer::{
    HarvestOpportunity, HarvestResult, HarvestSummary, HarvestingConfig, HarvestingEngine,
    SubstituteFinder, SubstituteSecurity, TaxCalculator, TaxJurisdiction, TaxLot, TaxRules,
    WashSaleCalendar, WashSaleMonitor, WashSaleSummary, WashSaleViolation, WashSaleWindow,
    YearEndSummary,
};

use crate::{get_default_analysis, ApiResponse, AppError, AppState};

/// Query params for jurisdiction
#[derive(Deserialize)]
pub struct JurisdictionQuery {
    pub jurisdiction: Option<String>,
}

/// Query for year-end summary
#[derive(Deserialize)]
pub struct YearEndQuery {
    pub year: Option<i32>,
    pub jurisdiction: Option<String>,
}

/// Request to add a tax lot
#[derive(Deserialize)]
pub struct AddLotRequest {
    pub symbol: String,
    pub shares: f64,
    pub cost_basis_per_share: f64,
    pub purchase_date: String,
}

/// Request to record a sale
#[derive(Deserialize)]
pub struct RecordSaleRequest {
    pub lot_id: String,
    pub sale_price: f64,
    pub sale_date: String,
}

/// Request to execute harvest
#[derive(Deserialize)]
pub struct ExecuteHarvestRequest {
    pub symbol: String,
    pub lot_id: String,
    pub substitute_symbol: Option<String>,
}

/// Response with tax settings
#[derive(Serialize)]
pub struct TaxSettingsResponse {
    pub jurisdiction: TaxJurisdiction,
    pub rules: TaxRules,
    pub supported_jurisdictions: Vec<JurisdictionInfo>,
}

#[derive(Serialize)]
pub struct JurisdictionInfo {
    pub code: String,
    pub name: String,
    pub has_wash_sale: bool,
    pub long_term_days: u32,
}

/// Response with harvest opportunities
#[derive(Serialize)]
pub struct HarvestOpportunitiesResponse {
    pub opportunities: Vec<HarvestOpportunity>,
    pub summary: HarvestSummary,
}

pub fn tax_routes() -> Router<AppState> {
    Router::new()
        // Tax settings and rules
        .route("/api/tax/settings", get(get_tax_settings))
        .route("/api/tax/rules/:jurisdiction", get(get_jurisdiction_rules))
        // Harvest opportunities
        .route("/api/tax/harvest-opportunities", get(get_harvest_opportunities))
        .route("/api/tax/harvest-opportunities/:symbol", get(get_symbol_opportunities))
        .route("/api/tax/execute-harvest", post(execute_harvest))
        // Substitutes
        .route("/api/tax/substitutes/:symbol", get(get_substitutes))
        // Wash sales
        .route("/api/tax/wash-sales", get(get_wash_sales))
        .route("/api/tax/wash-sales/:symbol", get(get_symbol_wash_sales))
        .route("/api/tax/wash-sale-calendar/:symbol", get(get_wash_sale_calendar))
        // Tax lots
        .route("/api/tax/lots", get(get_tax_lots))
        .route("/api/tax/lots", post(add_tax_lot))
        .route("/api/tax/lots/:id/sell", post(sell_tax_lot))
        // Summaries
        .route("/api/tax/year-end-summary", get(get_year_end_summary))
}

/// Get tax settings for current configuration
async fn get_tax_settings(
    Query(query): Query<JurisdictionQuery>,
) -> Result<Json<ApiResponse<TaxSettingsResponse>>, AppError> {
    let jurisdiction = parse_jurisdiction(query.jurisdiction.as_deref());
    let rules = TaxRules::for_jurisdiction(jurisdiction);

    let supported = vec![
        JurisdictionInfo {
            code: "US".to_string(),
            name: "United States".to_string(),
            has_wash_sale: true,
            long_term_days: 365,
        },
        JurisdictionInfo {
            code: "UK".to_string(),
            name: "United Kingdom".to_string(),
            has_wash_sale: true,
            long_term_days: 0,
        },
        JurisdictionInfo {
            code: "CA".to_string(),
            name: "Canada".to_string(),
            has_wash_sale: true,
            long_term_days: 0,
        },
        JurisdictionInfo {
            code: "AU".to_string(),
            name: "Australia".to_string(),
            has_wash_sale: false,
            long_term_days: 365,
        },
        JurisdictionInfo {
            code: "DE".to_string(),
            name: "Germany".to_string(),
            has_wash_sale: false,
            long_term_days: 0,
        },
    ];

    Ok(Json(ApiResponse::success(TaxSettingsResponse {
        jurisdiction,
        rules,
        supported_jurisdictions: supported,
    })))
}

/// Get rules for a specific jurisdiction
async fn get_jurisdiction_rules(
    Path(jurisdiction): Path<String>,
) -> Result<Json<ApiResponse<TaxRules>>, AppError> {
    let jurisdiction = parse_jurisdiction(Some(&jurisdiction));
    let rules = TaxRules::for_jurisdiction(jurisdiction);
    Ok(Json(ApiResponse::success(rules)))
}

/// Get all harvest opportunities
async fn get_harvest_opportunities(
    State(state): State<AppState>,
    Query(query): Query<JurisdictionQuery>,
) -> Result<Json<ApiResponse<HarvestOpportunitiesResponse>>, AppError> {
    let jurisdiction = parse_jurisdiction(query.jurisdiction.as_deref());
    let calculator = TaxCalculator::new(jurisdiction);
    let engine = HarvestingEngine::new(calculator);

    // Get tax lots from portfolio (would come from database)
    let lots = get_portfolio_lots(&state).await?;

    // Get current prices
    let mut prices = HashMap::new();
    for lot in &lots {
        if let Ok(analysis) = get_default_analysis(&state,&lot.symbol).await {
            if let Some(price) = analysis.current_price {
                prices.insert(lot.symbol.clone(), price);
            }
        }
    }

    // Find opportunities
    let opportunities = engine.find_opportunities(&lots, &prices);

    // Add substitutes to each opportunity
    let finder = SubstituteFinder::new();
    let opportunities_with_subs: Vec<HarvestOpportunity> = opportunities
        .into_iter()
        .map(|mut opp| {
            opp.substitutes = finder.find_substitutes(&opp.symbol);
            opp
        })
        .collect();

    let summary = engine.get_summary(&opportunities_with_subs);

    Ok(Json(ApiResponse::success(HarvestOpportunitiesResponse {
        opportunities: opportunities_with_subs,
        summary,
    })))
}

/// Get opportunities for a specific symbol
async fn get_symbol_opportunities(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<JurisdictionQuery>,
) -> Result<Json<ApiResponse<Vec<HarvestOpportunity>>>, AppError> {
    let symbol = symbol.to_uppercase();
    let jurisdiction = parse_jurisdiction(query.jurisdiction.as_deref());
    let calculator = TaxCalculator::new(jurisdiction);
    let engine = HarvestingEngine::new(calculator);

    let lots = get_portfolio_lots(&state).await?;
    let symbol_lots: Vec<_> = lots.into_iter().filter(|l| l.symbol == symbol).collect();

    if symbol_lots.is_empty() {
        return Ok(Json(ApiResponse::success(Vec::new())));
    }

    // Get current price
    let mut prices = HashMap::new();
    if let Ok(analysis) = get_default_analysis(&state,&symbol).await {
        if let Some(price) = analysis.current_price {
            prices.insert(symbol.clone(), price);
        }
    }

    let opportunities = engine.find_opportunities(&symbol_lots, &prices);

    let finder = SubstituteFinder::new();
    let opportunities_with_subs: Vec<HarvestOpportunity> = opportunities
        .into_iter()
        .map(|mut opp| {
            opp.substitutes = finder.find_substitutes(&opp.symbol);
            opp
        })
        .collect();

    Ok(Json(ApiResponse::success(opportunities_with_subs)))
}

/// Execute a harvest (simulation)
async fn execute_harvest(
    State(state): State<AppState>,
    Query(query): Query<JurisdictionQuery>,
    Json(request): Json<ExecuteHarvestRequest>,
) -> Result<Json<ApiResponse<HarvestResult>>, AppError> {
    let jurisdiction = parse_jurisdiction(query.jurisdiction.as_deref());
    let calculator = TaxCalculator::new(jurisdiction);
    let engine = HarvestingEngine::new(calculator);
    let finder = SubstituteFinder::new();

    // Find the opportunity
    let lots = get_portfolio_lots(&state).await?;
    let lot = lots
        .iter()
        .find(|l| l.id == request.lot_id && l.symbol == request.symbol)
        .ok_or_else(|| anyhow::anyhow!("Lot not found"))?;

    // Get current price
    let current_price = state
        .orchestrator
        .analyze(&request.symbol, analysis_core::Timeframe::Day1, 365)
        .await?
        .current_price
        .ok_or_else(|| anyhow::anyhow!("Could not get current price"))?;

    // Create opportunity from lot
    let mut prices = HashMap::new();
    prices.insert(request.symbol.clone(), current_price);

    let opportunities = engine.find_opportunities(&[lot.clone()], &prices);
    let opportunity = opportunities
        .first()
        .ok_or_else(|| anyhow::anyhow!("No harvest opportunity found for this lot"))?;

    // Get substitute if specified
    let substitute = request.substitute_symbol.as_ref().and_then(|sym| {
        finder
            .find_substitutes(&request.symbol)
            .into_iter()
            .find(|s| s.symbol == *sym)
    });

    let result = engine.simulate_harvest(opportunity, substitute.as_ref());

    Ok(Json(ApiResponse::success(result)))
}

/// Get substitutes for a symbol
async fn get_substitutes(
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<Vec<SubstituteSecurity>>>, AppError> {
    let finder = SubstituteFinder::new();
    let substitutes = finder.find_substitutes(&symbol.to_uppercase());
    Ok(Json(ApiResponse::success(substitutes)))
}

/// Get all wash sale windows/violations
async fn get_wash_sales(
    Query(query): Query<JurisdictionQuery>,
) -> Result<Json<ApiResponse<WashSalesResponse>>, AppError> {
    let jurisdiction = parse_jurisdiction(query.jurisdiction.as_deref());
    let rules = TaxRules::for_jurisdiction(jurisdiction);
    let monitor = WashSaleMonitor::new(rules);

    // In production, this would load from database
    let windows = monitor.all_windows().to_vec();
    let violations = monitor.all_violations().to_vec();
    let year = chrono::Utc::now().year();
    let summary = monitor.year_summary(year);

    Ok(Json(ApiResponse::success(WashSalesResponse {
        windows,
        violations,
        summary,
    })))
}

#[derive(Serialize)]
pub struct WashSalesResponse {
    pub windows: Vec<WashSaleWindow>,
    pub violations: Vec<WashSaleViolation>,
    pub summary: WashSaleSummary,
}

/// Get wash sales for a specific symbol
async fn get_symbol_wash_sales(
    Path(symbol): Path<String>,
    Query(query): Query<JurisdictionQuery>,
) -> Result<Json<ApiResponse<Vec<WashSaleWindow>>>, AppError> {
    let jurisdiction = parse_jurisdiction(query.jurisdiction.as_deref());
    let rules = TaxRules::for_jurisdiction(jurisdiction);
    let monitor = WashSaleMonitor::new(rules);

    let today = chrono::Utc::now().date_naive();
    let windows: Vec<_> = monitor
        .get_active_windows(&symbol.to_uppercase(), today)
        .into_iter()
        .cloned()
        .collect();

    Ok(Json(ApiResponse::success(windows)))
}

/// Get wash sale calendar for a symbol
async fn get_wash_sale_calendar(
    Path(symbol): Path<String>,
    Query(query): Query<JurisdictionQuery>,
) -> Result<Json<ApiResponse<WashSaleCalendar>>, AppError> {
    let jurisdiction = parse_jurisdiction(query.jurisdiction.as_deref());
    let rules = TaxRules::for_jurisdiction(jurisdiction);
    let monitor = WashSaleMonitor::new(rules);

    let calendar = monitor.get_calendar(&symbol.to_uppercase());

    Ok(Json(ApiResponse::success(calendar)))
}

/// Get all tax lots
async fn get_tax_lots(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<TaxLot>>>, AppError> {
    let lots = get_portfolio_lots(&state).await?;
    Ok(Json(ApiResponse::success(lots)))
}

/// Add a new tax lot
async fn add_tax_lot(
    Json(request): Json<AddLotRequest>,
) -> Result<Json<ApiResponse<TaxLot>>, AppError> {
    let purchase_date = chrono::NaiveDate::parse_from_str(&request.purchase_date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid date format: {}", e))?;

    let lot = TaxLot::new(
        uuid::Uuid::new_v4().to_string(),
        request.symbol.to_uppercase(),
        request.shares,
        request.cost_basis_per_share,
        purchase_date,
    );

    // In production, save to database
    Ok(Json(ApiResponse::success(lot)))
}

/// Sell a tax lot
async fn sell_tax_lot(
    Path(id): Path<String>,
    Json(request): Json<RecordSaleRequest>,
) -> Result<Json<ApiResponse<TaxLot>>, AppError> {
    let _sale_date = chrono::NaiveDate::parse_from_str(&request.sale_date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid date format: {}", e))?;

    // In production, update lot in database
    // For now, return error as we don't have persistence
    Err(anyhow::anyhow!("Lot {} not found", id).into())
}

/// Get year-end tax summary
async fn get_year_end_summary(
    State(state): State<AppState>,
    Query(query): Query<YearEndQuery>,
) -> Result<Json<ApiResponse<YearEndSummary>>, AppError> {
    let year = query.year.unwrap_or_else(|| chrono::Utc::now().year());
    let jurisdiction = parse_jurisdiction(query.jurisdiction.as_deref());

    let calculator = TaxCalculator::new(jurisdiction);

    // Get closed lots (would come from database)
    let lots = get_portfolio_lots(&state).await?;
    let closed_lots: Vec<_> = lots.into_iter().filter(|l| l.is_closed).collect();

    let summary = calculator.calculate_year_end_summary(year, &closed_lots, 0.0, 0.0);

    Ok(Json(ApiResponse::success(summary)))
}

// Helper functions

fn parse_jurisdiction(code: Option<&str>) -> TaxJurisdiction {
    match code.map(|s| s.to_uppercase()).as_deref() {
        Some("US") | Some("USA") => TaxJurisdiction::US,
        Some("UK") | Some("GB") => TaxJurisdiction::UK,
        Some("CA") | Some("CAN") | Some("CANADA") => TaxJurisdiction::Canada,
        Some("AU") | Some("AUS") | Some("AUSTRALIA") => TaxJurisdiction::Australia,
        Some("DE") | Some("DEU") | Some("GERMANY") => TaxJurisdiction::Germany,
        _ => TaxJurisdiction::US, // Default
    }
}

async fn get_portfolio_lots(state: &AppState) -> Result<Vec<TaxLot>, AppError> {
    let mut lots: Vec<TaxLot> = Vec::new();
    let mut seen_symbols = std::collections::HashSet::new();

    // Try Alpaca positions first (has real cost basis data)
    if let Some(alpaca) = &state.alpaca_client {
        if let Ok(positions) = alpaca.get_positions().await {
            for pos in &positions {
                let qty = pos.qty.parse::<f64>().unwrap_or(0.0);
                let entry_price = pos.avg_entry_price.parse::<f64>().unwrap_or(0.0);
                if qty > 0.0 && entry_price > 0.0 {
                    seen_symbols.insert(pos.symbol.clone());
                    lots.push(TaxLot::new(
                        uuid::Uuid::new_v4().to_string(),
                        pos.symbol.clone(),
                        qty,
                        entry_price,
                        // Alpaca doesn't expose purchase date per lot; estimate ~90 days
                        chrono::Utc::now().date_naive() - chrono::Duration::days(90),
                    ));
                }
            }
        }
    }

    // Also check local portfolio for positions not in Alpaca
    if let Some(pm) = &state.portfolio_manager {
        if let Ok(portfolio) = pm.get_portfolio().await {
            for pos in &portfolio {
                if !seen_symbols.contains(&pos.symbol) {
                    lots.push(TaxLot::new(
                        uuid::Uuid::new_v4().to_string(),
                        pos.symbol.clone(),
                        pos.shares,
                        pos.entry_price,
                        chrono::NaiveDate::parse_from_str(&pos.entry_date, "%Y-%m-%d")
                            .unwrap_or_else(|_| chrono::Utc::now().date_naive() - chrono::Duration::days(180)),
                    ));
                }
            }
        }
    }

    Ok(lots)
}
