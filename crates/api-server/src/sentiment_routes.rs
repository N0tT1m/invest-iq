//! Sentiment Routes
//!
//! API endpoints for sentiment analysis including velocity tracking,
//! history retrieval, and narrative shift detection.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sentiment_analysis::{
    SentimentDataPoint, SentimentDynamics, SentimentVelocityCalculator, VelocitySignal,
};
use std::collections::BTreeMap;

use sqlx;

use crate::{get_default_analysis, ApiResponse, AppError, AppState};

/// Query parameters for sentiment velocity
#[derive(Deserialize)]
pub struct VelocityQuery {
    /// Number of days of history to analyze (default: 7)
    #[serde(default = "default_days")]
    pub days: i64,
}

fn default_days() -> i64 {
    7
}

/// Query parameters for sentiment history
#[derive(Deserialize)]
pub struct HistoryQuery {
    /// Number of days of history to return (default: 30)
    #[serde(default = "default_history_days")]
    pub days: i64,
    /// Limit number of records (default: 100)
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_history_days() -> i64 {
    30
}

fn default_limit() -> i64 {
    100
}

/// Request to record a sentiment data point
#[derive(Deserialize)]
pub struct RecordSentimentRequest {
    pub symbol: String,
    pub sentiment_score: f64,
    pub article_count: i32,
    #[serde(default)]
    pub timestamp: Option<DateTime<Utc>>,
}

/// Response for sentiment velocity
#[derive(Serialize)]
pub struct SentimentVelocityResponse {
    pub symbol: String,
    pub dynamics: SentimentDynamics,
    pub history_points: usize,
    pub analysis_window_days: i64,
}

/// Response for sentiment history
#[derive(Serialize)]
pub struct SentimentHistoryResponse {
    pub symbol: String,
    pub history: Vec<SentimentHistoryPoint>,
    pub summary: SentimentSummary,
}

/// A point in sentiment history
#[derive(Serialize)]
pub struct SentimentHistoryPoint {
    pub timestamp: DateTime<Utc>,
    pub sentiment_score: f64,
    pub article_count: i32,
    pub velocity: Option<f64>,
    pub signal: Option<String>,
}

/// Summary statistics for sentiment
#[derive(Serialize)]
pub struct SentimentSummary {
    pub avg_sentiment: f64,
    pub min_sentiment: f64,
    pub max_sentiment: f64,
    pub total_articles: i32,
    pub trend: String, // "improving", "declining", "stable"
}

/// Create sentiment routes
pub fn sentiment_routes() -> Router<AppState> {
    Router::new()
        .route("/api/sentiment/:symbol/velocity", get(get_sentiment_velocity))
        .route("/api/sentiment/:symbol/history", get(get_sentiment_history))
        .route("/api/sentiment/record", post(record_sentiment))
        .route("/api/sentiment/:symbol/analyze", get(analyze_sentiment_now))
        .route("/api/sentiment/:symbol/social", get(get_social_sentiment))
}

/// A headline entry with sentiment
#[derive(Serialize)]
pub struct HeadlineEntry {
    pub title: String,
    pub published: String,
    pub sentiment: f64,
    pub sentiment_label: String,
    pub url: String,
}

/// Social sentiment response (news-powered)
#[derive(Serialize)]
pub struct SocialSentimentResponse {
    pub symbol: String,
    pub available: bool,
    pub news_mentions: i32,
    pub sentiment_score: f64,
    pub sentiment_label: String,
    pub buzz_score: f64,
    pub positive_pct: f64,
    pub negative_pct: f64,
    pub neutral_pct: f64,
    pub top_headlines: Vec<HeadlineEntry>,
    pub message: Option<String>,
}

/// Positive / negative word lists for simple news scoring
const POSITIVE_WORDS: &[&str] = &[
    "upgrade", "beat", "surge", "rally", "gain", "growth", "profit", "bullish",
    "outperform", "strong", "record", "high", "positive", "optimistic", "buy",
    "boost", "rise", "jump", "soar", "breakout", "momentum", "upbeat", "exceeds",
    "dividend", "innovative", "partnership", "expansion", "recovery",
];

const NEGATIVE_WORDS: &[&str] = &[
    "downgrade", "miss", "plunge", "crash", "loss", "decline", "bearish",
    "underperform", "weak", "low", "negative", "pessimistic", "sell",
    "drop", "fall", "slump", "warning", "risk", "lawsuit", "fraud",
    "bankruptcy", "default", "layoff", "cut", "recession", "investigation",
    "recall", "debt", "concern",
];

fn score_text(text: &str) -> f64 {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let mut score = 0.0f64;
    for w in &words {
        let clean: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
        if POSITIVE_WORDS.contains(&clean.as_str()) {
            score += 1.0;
        }
        if NEGATIVE_WORDS.contains(&clean.as_str()) {
            score -= 1.0;
        }
    }
    score
}

/// Get social sentiment (news-powered)
async fn get_social_sentiment(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<SocialSentimentResponse>>, AppError> {
    let symbol = symbol.to_uppercase();
    let articles = state.orchestrator.get_news(&symbol, 30).await.unwrap_or_default();

    if articles.is_empty() {
        return Ok(Json(ApiResponse {
            success: true,
            data: Some(SocialSentimentResponse {
                symbol,
                available: false,
                news_mentions: 0,
                sentiment_score: 0.0,
                sentiment_label: "Neutral".to_string(),
                buzz_score: 0.0,
                positive_pct: 0.0,
                negative_pct: 0.0,
                neutral_pct: 100.0,
                top_headlines: vec![],
                message: Some("No recent news found for this symbol.".to_string()),
            }),
            error: None,
        }));
    }

    let count = articles.len() as f64;
    let mut total_score = 0.0f64;
    let mut positive = 0i32;
    let mut negative = 0i32;
    let mut headlines: Vec<HeadlineEntry> = Vec::new();

    for article in &articles {
        let text = format!(
            "{} {}",
            article.title,
            article.description.as_deref().unwrap_or("")
        );
        let s = score_text(&text);
        total_score += s;

        let (label, bucket) = if s > 0.5 {
            positive += 1;
            ("Positive", s)
        } else if s < -0.5 {
            negative += 1;
            ("Negative", s)
        } else {
            ("Neutral", s)
        };

        if headlines.len() < 5 {
            headlines.push(HeadlineEntry {
                title: article.title.clone(),
                published: article.published_utc.format("%Y-%m-%d %H:%M").to_string(),
                sentiment: (bucket * 100.0 / 3.0).clamp(-100.0, 100.0),
                sentiment_label: label.to_string(),
                url: article.article_url.clone(),
            });
        }
    }

    // Normalize sentiment to -100..+100
    let avg_score = total_score / count;
    let sentiment_score = (avg_score * 33.3).clamp(-100.0, 100.0);
    let sentiment_label = if sentiment_score > 20.0 {
        "Bullish"
    } else if sentiment_score < -20.0 {
        "Bearish"
    } else {
        "Neutral"
    };

    // Buzz: articles / baseline of 5
    let buzz_score = (count / 5.0 * 100.0).min(200.0);

    let positive_pct = (positive as f64 / count * 100.0).round();
    let negative_pct = (negative as f64 / count * 100.0).round();
    let neutral_pct = (100.0 - positive_pct - negative_pct).max(0.0);

    Ok(Json(ApiResponse {
        success: true,
        data: Some(SocialSentimentResponse {
            symbol,
            available: true,
            news_mentions: articles.len() as i32,
            sentiment_score: (sentiment_score * 10.0).round() / 10.0,
            sentiment_label: sentiment_label.to_string(),
            buzz_score: (buzz_score * 10.0).round() / 10.0,
            positive_pct,
            negative_pct,
            neutral_pct,
            top_headlines: headlines,
            message: Some("Powered by Polygon News".to_string()),
        }),
        error: None,
    }))
}

/// Get sentiment velocity for a symbol
///
/// Calculates the rate of change in sentiment and provides
/// trading signals based on sentiment momentum.
async fn get_sentiment_velocity(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<VelocityQuery>,
) -> Result<Json<ApiResponse<SentimentVelocityResponse>>, AppError> {
    let symbol = symbol.to_uppercase();

    // Get sentiment history from database
    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?;

    let mut history = get_sentiment_history_from_db(&portfolio_manager, &symbol, query.days).await?;

    // Auto-record: if no recent data point (within 1 hour), save current sentiment
    let needs_recording = if let Some(latest) = history.last() {
        Utc::now() - latest.timestamp > Duration::hours(1)
    } else {
        true
    };

    if needs_recording {
        if let Ok(analysis) = get_default_analysis(&state, &symbol).await {
            let sentiment_score = analysis
                .sentiment
                .as_ref()
                .and_then(|s| s.metrics.get("normalized_score"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let article_count = analysis
                .sentiment
                .as_ref()
                .and_then(|s| s.metrics.get("total_articles"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;

            let now = Utc::now();
            let _ = save_sentiment_to_db(
                &portfolio_manager,
                &symbol,
                now,
                sentiment_score,
                article_count,
                None,
                None,
                None,
            )
            .await;

            // Add the new point to our in-memory history so velocity calc includes it
            history.push(SentimentDataPoint {
                timestamp: now,
                sentiment_score,
                article_count,
                symbol: symbol.clone(),
            });
        }
    }

    // Seed from news articles if we have fewer than 3 points (minimum for velocity calc)
    if history.len() < 3 {
        let articles = state.orchestrator.get_news(&symbol, 50).await.unwrap_or_default();
        if !articles.is_empty() {
            // Group articles by date and compute daily sentiment
            let mut daily: BTreeMap<NaiveDate, (f64, i32)> = BTreeMap::new();
            for article in &articles {
                let date = article.published_utc.date_naive();
                let text = format!(
                    "{} {}",
                    article.title,
                    article.description.as_deref().unwrap_or("")
                );
                let s = score_text(&text);
                let entry = daily.entry(date).or_insert((0.0, 0));
                entry.0 += s;
                entry.1 += 1;
            }

            // Convert to data points and save any we don't already have
            let existing_dates: std::collections::HashSet<NaiveDate> = history
                .iter()
                .map(|p| p.timestamp.date_naive())
                .collect();

            for (date, (total_score, count)) in &daily {
                if existing_dates.contains(date) {
                    continue;
                }
                let avg_score = *total_score / (*count as f64);
                // Normalize to -100..100 scale (word-list scores are typically -3..3)
                let normalized = (avg_score * 33.3).clamp(-100.0, 100.0);
                let ts = date
                    .and_hms_opt(12, 0, 0)
                    .unwrap()
                    .and_utc();

                let _ = save_sentiment_to_db(
                    &portfolio_manager,
                    &symbol,
                    ts,
                    normalized,
                    *count,
                    None,
                    None,
                    None,
                )
                .await;

                history.push(SentimentDataPoint {
                    timestamp: ts,
                    sentiment_score: normalized,
                    article_count: *count,
                    symbol: symbol.clone(),
                });
            }

            // Sort by timestamp after adding seeded points
            history.sort_by_key(|p| p.timestamp);
        }
    }

    if history.is_empty() {
        return Ok(Json(ApiResponse::success(SentimentVelocityResponse {
            symbol: symbol.clone(),
            dynamics: SentimentDynamics {
                current_sentiment: 0.0,
                velocity: 0.0,
                acceleration: 0.0,
                narrative_shift: None,
                signal: VelocitySignal::Stable,
                interpretation: "No sentiment data available. Try again after analyzing the symbol.".to_string(),
                confidence: 0.0,
            },
            history_points: 0,
            analysis_window_days: query.days,
        })));
    }

    // Calculate velocity
    let calculator = SentimentVelocityCalculator::default();
    let dynamics = calculator.calculate(&history);

    Ok(Json(ApiResponse::success(SentimentVelocityResponse {
        symbol,
        dynamics,
        history_points: history.len(),
        analysis_window_days: query.days,
    })))
}

/// Get sentiment history for a symbol
async fn get_sentiment_history(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<ApiResponse<SentimentHistoryResponse>>, AppError> {
    let symbol = symbol.to_uppercase();

    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?;

    let history = get_sentiment_history_from_db(&portfolio_manager, &symbol, query.days).await?;

    // Convert to response format
    let history_points: Vec<SentimentHistoryPoint> = history
        .iter()
        .take(query.limit as usize)
        .map(|p| SentimentHistoryPoint {
            timestamp: p.timestamp,
            sentiment_score: p.sentiment_score,
            article_count: p.article_count,
            velocity: None, // Could calculate per-point velocity
            signal: None,
        })
        .collect();

    // Calculate summary
    let summary = if history.is_empty() {
        SentimentSummary {
            avg_sentiment: 0.0,
            min_sentiment: 0.0,
            max_sentiment: 0.0,
            total_articles: 0,
            trend: "unknown".to_string(),
        }
    } else {
        let scores: Vec<f64> = history.iter().map(|p| p.sentiment_score).collect();
        let avg = scores.iter().sum::<f64>() / scores.len() as f64;
        let min = scores.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let total_articles: i32 = history.iter().map(|p| p.article_count).sum();

        // Determine trend
        let trend = if history.len() >= 2 {
            let first_half_avg: f64 = scores[..scores.len() / 2].iter().sum::<f64>()
                / (scores.len() / 2) as f64;
            let second_half_avg: f64 = scores[scores.len() / 2..].iter().sum::<f64>()
                / (scores.len() - scores.len() / 2) as f64;

            if second_half_avg > first_half_avg + 10.0 {
                "improving"
            } else if second_half_avg < first_half_avg - 10.0 {
                "declining"
            } else {
                "stable"
            }
        } else {
            "insufficient_data"
        };

        SentimentSummary {
            avg_sentiment: avg,
            min_sentiment: min,
            max_sentiment: max,
            total_articles,
            trend: trend.to_string(),
        }
    };

    Ok(Json(ApiResponse::success(SentimentHistoryResponse {
        symbol,
        history: history_points,
        summary,
    })))
}

/// Record a sentiment data point
async fn record_sentiment(
    State(state): State<AppState>,
    Json(req): Json<RecordSentimentRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let symbol = req.symbol.to_uppercase();
    let timestamp = req.timestamp.unwrap_or_else(Utc::now);

    let portfolio_manager = state.portfolio_manager.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Database not configured"))?;

    // Calculate velocity if we have history
    let history = get_sentiment_history_from_db(&portfolio_manager, &symbol, 7).await?;

    let (velocity, acceleration, signal) = if !history.is_empty() {
        let mut all_points = history.clone();
        all_points.push(SentimentDataPoint {
            timestamp,
            sentiment_score: req.sentiment_score,
            article_count: req.article_count,
            symbol: symbol.clone(),
        });

        let calculator = SentimentVelocityCalculator::default();
        let dynamics = calculator.calculate(&all_points);
        (
            Some(dynamics.velocity),
            Some(dynamics.acceleration),
            Some(dynamics.signal.as_str().to_string()),
        )
    } else {
        (None, None, None)
    };

    // Insert into database
    save_sentiment_to_db(
        &portfolio_manager,
        &symbol,
        timestamp,
        req.sentiment_score,
        req.article_count,
        velocity,
        acceleration,
        signal,
    )
    .await?;

    Ok(Json(ApiResponse::success(format!(
        "Sentiment recorded for {}",
        symbol
    ))))
}

/// Analyze current sentiment and record it
async fn analyze_sentiment_now(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<SentimentVelocityResponse>>, AppError> {
    let symbol = symbol.to_uppercase();

    // Get fresh analysis
    let analysis = get_default_analysis(&state, &symbol).await?;

    // Extract sentiment data
    let sentiment_score = analysis
        .sentiment
        .as_ref()
        .and_then(|s| s.metrics.get("normalized_score"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let article_count = analysis
        .sentiment
        .as_ref()
        .and_then(|s| s.metrics.get("total_articles"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    // Record it
    if let Some(portfolio_manager) = state.portfolio_manager.as_ref() {
        let _ = save_sentiment_to_db(
            portfolio_manager,
            &symbol,
            Utc::now(),
            sentiment_score,
            article_count,
            None,
            None,
            None,
        )
        .await;

        // Get updated history and calculate velocity
        let history = get_sentiment_history_from_db(portfolio_manager, &symbol, 7).await?;
        let calculator = SentimentVelocityCalculator::default();
        let dynamics = calculator.calculate(&history);

        return Ok(Json(ApiResponse::success(SentimentVelocityResponse {
            symbol,
            dynamics,
            history_points: history.len(),
            analysis_window_days: 7,
        })));
    }

    // No database - just return current sentiment
    Ok(Json(ApiResponse::success(SentimentVelocityResponse {
        symbol,
        dynamics: SentimentDynamics {
            current_sentiment: sentiment_score,
            velocity: 0.0,
            acceleration: 0.0,
            narrative_shift: None,
            signal: VelocitySignal::Stable,
            interpretation: format!("Current sentiment: {:.1}. Enable database for velocity tracking.", sentiment_score),
            confidence: 0.0,
        },
        history_points: 0,
        analysis_window_days: 7,
    })))
}

// Helper functions for database operations

/// Row type for reading sentiment_history table
#[derive(sqlx::FromRow)]
struct SentimentHistoryRow {
    symbol: String,
    timestamp: String,
    sentiment_score: f64,
    article_count: Option<i32>,
}

async fn get_sentiment_history_from_db(
    portfolio_manager: &portfolio_manager::PortfolioManager,
    symbol: &str,
    days: i64,
) -> Result<Vec<SentimentDataPoint>, anyhow::Error> {
    let rows = sqlx::query_as::<_, SentimentHistoryRow>(
        "SELECT symbol, timestamp, sentiment_score, article_count FROM sentiment_history \
         WHERE symbol = ? AND timestamp >= datetime('now', '-' || ? || ' days') \
         ORDER BY timestamp ASC"
    )
    .bind(symbol)
    .bind(days)
    .fetch_all(portfolio_manager.db().pool())
    .await?;

    let mut points = Vec::new();
    for row in rows {
        // Parse timestamp - handle both RFC3339 and SQLite CURRENT_TIMESTAMP formats
        let ts = chrono::DateTime::parse_from_rfc3339(&row.timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(&row.timestamp, "%Y-%m-%d %H:%M:%S")
                    .map(|naive| naive.and_utc())
            })
            .unwrap_or_else(|_| Utc::now());

        points.push(SentimentDataPoint {
            timestamp: ts,
            sentiment_score: row.sentiment_score,
            article_count: row.article_count.unwrap_or(0),
            symbol: row.symbol,
        });
    }

    Ok(points)
}

async fn save_sentiment_to_db(
    portfolio_manager: &portfolio_manager::PortfolioManager,
    symbol: &str,
    timestamp: DateTime<Utc>,
    sentiment_score: f64,
    article_count: i32,
    velocity: Option<f64>,
    acceleration: Option<f64>,
    signal: Option<String>,
) -> Result<(), anyhow::Error> {
    sqlx::query(
        "INSERT INTO sentiment_history (symbol, timestamp, sentiment_score, article_count, velocity, acceleration, signal) \
         VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(symbol)
    .bind(timestamp.to_rfc3339())
    .bind(sentiment_score)
    .bind(article_count)
    .bind(velocity)
    .bind(acceleration)
    .bind(signal)
    .execute(portfolio_manager.db().pool())
    .await?;

    Ok(())
}
