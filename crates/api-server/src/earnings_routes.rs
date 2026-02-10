use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::{ApiResponse, AppError, AppState};

#[derive(Serialize)]
pub struct EarningsNewsItem {
    pub title: String,
    pub published: String,
    pub url: String,
}

#[derive(Serialize)]
pub struct EarningsData {
    pub symbol: String,
    pub data_source: String,
    pub historical: Vec<EarningsQuarter>,
    pub eps_growth_rate: Option<f64>,
    pub revenue_growth_rate: Option<f64>,
    pub beat_rate: Option<f64>,
    pub earnings_news: Vec<EarningsNewsItem>,
}

#[derive(Serialize)]
pub struct EarningsQuarter {
    pub fiscal_period: String,
    pub fiscal_year: i32,
    pub eps: Option<f64>,
    pub revenue: Option<f64>,
    pub revenue_formatted: Option<String>,
    pub eps_change_qoq: Option<f64>,
    pub revenue_growth_yoy: Option<f64>,
}

pub fn earnings_routes() -> Router<AppState> {
    Router::new()
        .route("/api/earnings/:symbol", get(get_earnings))
}

const EARNINGS_KEYWORDS: &[&str] = &[
    "earnings", "eps", "revenue", "quarterly", "profit", "income",
    "beat", "miss", "guidance", "forecast", "results",
];

fn is_earnings_related(title: &str, desc: &str) -> bool {
    let text = format!("{} {}", title, desc).to_lowercase();
    EARNINGS_KEYWORDS.iter().any(|kw| text.contains(kw))
}

async fn get_earnings(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<EarningsData>>, AppError> {
    let financials = state.orchestrator.get_financials(&symbol).await.unwrap_or_default();

    if financials.is_empty() {
        // Fallback: fetch earnings-related news
        let articles = state.orchestrator.get_news(&symbol, 30).await.unwrap_or_default();
        let earnings_news: Vec<EarningsNewsItem> = articles.iter()
            .filter(|a| is_earnings_related(&a.title, a.description.as_deref().unwrap_or("")))
            .take(10)
            .map(|a| EarningsNewsItem {
                title: a.title.clone(),
                published: a.published_utc.format("%Y-%m-%d").to_string(),
                url: a.article_url.clone(),
            })
            .collect();

        return Ok(Json(ApiResponse {
            success: true,
            data: Some(EarningsData {
                symbol,
                data_source: "news_fallback".to_string(),
                historical: vec![],
                eps_growth_rate: None,
                revenue_growth_rate: None,
                beat_rate: None,
                earnings_news,
            }),
            error: None,
        }));
    }

    // Full financials path (premium)
    let mut quarters: Vec<EarningsQuarter> = Vec::new();
    let financials_sorted: Vec<_> = financials.iter().rev().collect();

    for (i, f) in financials_sorted.iter().enumerate() {
        let eps_change_qoq = if i > 0 {
            match (f.eps, financials_sorted[i - 1].eps) {
                (Some(curr), Some(prev)) if prev != 0.0 => Some(((curr - prev) / prev.abs()) * 100.0),
                _ => None,
            }
        } else {
            None
        };

        let revenue_growth_yoy = if i >= 4 {
            match (f.revenue, financials_sorted[i - 4].revenue) {
                (Some(curr), Some(prev)) if prev != 0.0 => Some(((curr - prev) / prev.abs()) * 100.0),
                _ => None,
            }
        } else {
            None
        };

        let revenue_formatted = f.revenue.map(|r| {
            if r.abs() >= 1e9 { format!("${:.2}B", r / 1e9) }
            else if r.abs() >= 1e6 { format!("${:.1}M", r / 1e6) }
            else { format!("${:.0}", r) }
        });

        quarters.push(EarningsQuarter {
            fiscal_period: f.fiscal_period.clone(),
            fiscal_year: f.fiscal_year,
            eps: f.eps,
            revenue: f.revenue,
            revenue_formatted,
            eps_change_qoq,
            revenue_growth_yoy,
        });
    }

    let eps_growth_rate = if quarters.len() >= 2 {
        let first_eps = quarters.first().and_then(|q| q.eps);
        let last_eps = quarters.last().and_then(|q| q.eps);
        match (first_eps, last_eps) {
            (Some(first), Some(last)) if first != 0.0 => Some(((last - first) / first.abs()) * 100.0),
            _ => None,
        }
    } else {
        None
    };

    let revenue_growth_rate = if quarters.len() >= 2 {
        let first_rev = quarters.first().and_then(|q| q.revenue);
        let last_rev = quarters.last().and_then(|q| q.revenue);
        match (first_rev, last_rev) {
            (Some(first), Some(last)) if first != 0.0 => Some(((last - first) / first.abs()) * 100.0),
            _ => None,
        }
    } else {
        None
    };

    let beats: Vec<_> = quarters.iter().filter(|q| q.eps_change_qoq.map_or(false, |c| c > 0.0)).collect();
    let total_with_data: Vec<_> = quarters.iter().filter(|q| q.eps_change_qoq.is_some()).collect();
    let beat_rate = if !total_with_data.is_empty() {
        Some((beats.len() as f64 / total_with_data.len() as f64) * 100.0)
    } else {
        None
    };

    Ok(Json(ApiResponse {
        success: true,
        data: Some(EarningsData {
            symbol,
            data_source: "premium".to_string(),
            historical: quarters,
            eps_growth_rate,
            revenue_growth_rate,
            beat_rate,
            earnings_news: vec![],
        }),
        error: None,
    }))
}
