use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::{ApiResponse, AppError, AppState};

#[derive(Serialize, utoipa::ToSchema)]
pub struct DividendNewsItem {
    pub title: String,
    pub published: String,
    pub url: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct DividendAnalysis {
    pub symbol: String,
    pub data_source: String,
    pub has_dividends: bool,
    pub current_yield: Option<f64>,
    pub annual_dividend: Option<f64>,
    pub frequency: Option<String>,
    pub ex_dividend_date: Option<String>,
    pub pay_date: Option<String>,
    pub growth_rate: Option<f64>,
    pub history: Vec<DividendHistoryEntry>,
    pub dividend_news: Vec<DividendNewsItem>,
    pub message: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct DividendHistoryEntry {
    pub date: String,
    pub amount: f64,
    pub dividend_type: String,
}

pub fn dividend_routes() -> Router<AppState> {
    Router::new().route("/api/dividends/:symbol", get(get_dividends))
}

const DIVIDEND_KEYWORDS: &[&str] = &["dividend", "yield", "payout", "distribution", "ex-dividend"];

fn is_dividend_related(title: &str, desc: &str) -> bool {
    let text = format!("{} {}", title, desc).to_lowercase();
    DIVIDEND_KEYWORDS.iter().any(|kw| text.contains(kw))
}

#[utoipa::path(
    get,
    path = "/api/dividends/{symbol}",
    params(("symbol" = String, Path, description = "Stock ticker symbol")),
    responses((status = 200, description = "Dividend analysis with history and yield")),
    tag = "Market Data"
)]
async fn get_dividends(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<DividendAnalysis>>, AppError> {
    let dividends = state
        .orchestrator
        .get_dividends(&symbol)
        .await
        .unwrap_or_default();

    if dividends.is_empty() {
        // Fetch news for dividend-related headlines
        let articles = state
            .orchestrator
            .get_news(&symbol, 20)
            .await
            .unwrap_or_default();
        let dividend_news: Vec<DividendNewsItem> = articles
            .iter()
            .filter(|a| is_dividend_related(&a.title, a.description.as_deref().unwrap_or("")))
            .take(5)
            .map(|a| DividendNewsItem {
                title: a.title.clone(),
                published: a.published_utc.format("%Y-%m-%d").to_string(),
                url: a.article_url.clone(),
            })
            .collect();

        let message = if dividend_news.is_empty() {
            "Dividend data requires a premium Polygon.io plan. No dividend-related news found."
        } else {
            "Dividend data requires a premium Polygon.io plan. Showing related news."
        };

        return Ok(Json(ApiResponse {
            success: true,
            data: Some(DividendAnalysis {
                symbol,
                data_source: "news_fallback".to_string(),
                has_dividends: false,
                current_yield: None,
                annual_dividend: None,
                frequency: None,
                ex_dividend_date: None,
                pay_date: None,
                growth_rate: None,
                history: vec![],
                dividend_news,
                message: Some(message.to_string()),
            }),
            error: None,
        }));
    }

    // Full dividend data path (premium)
    let history: Vec<DividendHistoryEntry> = dividends
        .iter()
        .filter_map(|d| {
            let date = d.ex_dividend_date.clone()?;
            let amount = d.cash_amount?;
            Some(DividendHistoryEntry {
                date,
                amount,
                dividend_type: d.dividend_type.clone().unwrap_or_else(|| "CD".to_string()),
            })
        })
        .collect();

    let latest = &dividends[0];
    let ex_dividend_date = latest.ex_dividend_date.clone();
    let pay_date = latest.pay_date.clone();

    let frequency_str = match latest.frequency {
        Some(1) => Some("Annual".to_string()),
        Some(2) => Some("Semi-Annual".to_string()),
        Some(4) => Some("Quarterly".to_string()),
        Some(12) => Some("Monthly".to_string()),
        _ => None,
    };

    let freq = latest.frequency.unwrap_or(4) as f64;
    let latest_amount = latest.cash_amount.unwrap_or(0.0);
    let annual_dividend = latest_amount * freq;

    let current_yield = if annual_dividend > 0.0 {
        if let Ok(bars) = state
            .orchestrator
            .get_bars(&symbol, analysis_core::Timeframe::Day1, 5)
            .await
        {
            bars.last().map(|b| (annual_dividend / b.close) * 100.0)
        } else {
            None
        }
    } else {
        None
    };

    let amounts: Vec<f64> = dividends.iter().filter_map(|d| d.cash_amount).collect();
    let growth_rate = if amounts.len() >= 5 {
        let recent = amounts[0];
        let old = amounts[amounts.len() - 1];
        if old > 0.0 {
            Some(((recent / old).powf(1.0 / (amounts.len() as f64 - 1.0)) - 1.0) * 100.0)
        } else {
            None
        }
    } else {
        None
    };

    Ok(Json(ApiResponse {
        success: true,
        data: Some(DividendAnalysis {
            symbol,
            data_source: "premium".to_string(),
            has_dividends: true,
            current_yield,
            annual_dividend: Some(annual_dividend),
            frequency: frequency_str,
            ex_dividend_date,
            pay_date,
            growth_rate,
            history,
            dividend_news: vec![],
            message: None,
        }),
        error: None,
    }))
}
