use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::{ApiResponse, AppError, AppState};

#[derive(Serialize)]
pub struct InsiderNewsItem {
    pub title: String,
    pub published: String,
    pub url: String,
}

#[derive(Serialize)]
pub struct InsiderData {
    pub symbol: String,
    pub available: bool,
    pub data_source: String,
    pub transactions: Vec<InsiderTransactionEntry>,
    pub net_sentiment: Option<String>,
    pub total_buys: i32,
    pub total_sells: i32,
    pub net_value: Option<f64>,
    pub insider_news: Vec<InsiderNewsItem>,
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct InsiderTransactionEntry {
    pub date: String,
    pub name: String,
    pub title: String,
    pub transaction_type: String,
    pub shares: f64,
    pub price: Option<f64>,
    pub total_value: Option<f64>,
}

pub fn insider_routes() -> Router<AppState> {
    Router::new()
        .route("/api/insiders/:symbol", get(get_insiders))
}

const INSIDER_KEYWORDS: &[&str] = &[
    "insider", "ceo", "cfo", "director", "form 4", "bought", "sold",
    "executive", "officer", "chairman", "purchase", "acquisition",
];

fn is_insider_related(title: &str, desc: &str) -> bool {
    let text = format!("{} {}", title, desc).to_lowercase();
    INSIDER_KEYWORDS.iter().any(|kw| text.contains(kw))
}

async fn get_insiders(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<InsiderData>>, AppError> {
    let transactions = state.orchestrator.get_insider_transactions(&symbol).await.unwrap_or_default();

    if transactions.is_empty() {
        // Fallback: fetch insider-related news
        let articles = state.orchestrator.get_news(&symbol, 30).await.unwrap_or_default();
        let insider_news: Vec<InsiderNewsItem> = articles.iter()
            .filter(|a| is_insider_related(&a.title, a.description.as_deref().unwrap_or("")))
            .take(10)
            .map(|a| InsiderNewsItem {
                title: a.title.clone(),
                published: a.published_utc.format("%Y-%m-%d").to_string(),
                url: a.article_url.clone(),
            })
            .collect();

        let message = if insider_news.is_empty() {
            "Insider transaction data requires a premium Polygon.io plan. No insider-related news found."
        } else {
            "Insider transaction data requires a premium plan. Showing related news."
        };

        return Ok(Json(ApiResponse {
            success: true,
            data: Some(InsiderData {
                symbol,
                available: !insider_news.is_empty(),
                data_source: "news_fallback".to_string(),
                transactions: vec![],
                net_sentiment: None,
                total_buys: 0,
                total_sells: 0,
                net_value: None,
                insider_news,
                message: Some(message.to_string()),
            }),
            error: None,
        }));
    }

    // Full transaction path (premium)
    let mut entries = Vec::new();
    let mut total_buys = 0i32;
    let mut total_sells = 0i32;
    let mut buy_value = 0.0f64;
    let mut sell_value = 0.0f64;

    for t in &transactions {
        let tx_type = t.transaction_type.clone().unwrap_or_default();
        let is_buy = tx_type.to_lowercase().contains("purchase") || tx_type.to_lowercase().contains("buy");
        let is_sell = tx_type.to_lowercase().contains("sale") || tx_type.to_lowercase().contains("sell");

        if is_buy { total_buys += 1; }
        if is_sell { total_sells += 1; }

        let value = t.total_value.or_else(|| {
            match (t.shares, t.price_per_share) {
                (Some(s), Some(p)) => Some(s * p),
                _ => None,
            }
        });

        if is_buy { buy_value += value.unwrap_or(0.0); }
        if is_sell { sell_value += value.unwrap_or(0.0); }

        entries.push(InsiderTransactionEntry {
            date: t.filing_date.clone().unwrap_or_default(),
            name: t.name.clone().unwrap_or_else(|| "Unknown".to_string()),
            title: t.title.clone().unwrap_or_else(|| "N/A".to_string()),
            transaction_type: tx_type,
            shares: t.shares.unwrap_or(0.0),
            price: t.price_per_share,
            total_value: value,
        });
    }

    let net_value = buy_value - sell_value;
    let net_sentiment = if total_buys > total_sells * 2 {
        Some("Strongly Bullish".to_string())
    } else if total_buys > total_sells {
        Some("Bullish".to_string())
    } else if total_sells > total_buys * 2 {
        Some("Strongly Bearish".to_string())
    } else if total_sells > total_buys {
        Some("Bearish".to_string())
    } else {
        Some("Neutral".to_string())
    };

    Ok(Json(ApiResponse {
        success: true,
        data: Some(InsiderData {
            symbol,
            available: true,
            data_source: "premium".to_string(),
            transactions: entries,
            net_sentiment,
            total_buys,
            total_sells,
            net_value: Some(net_value),
            insider_news: vec![],
            message: None,
        }),
        error: None,
    }))
}
