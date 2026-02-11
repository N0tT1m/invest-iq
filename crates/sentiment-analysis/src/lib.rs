use analysis_core::{adaptive, AnalysisError, AnalysisResult, NewsArticle, SentimentAnalyzer, SignalStrength};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::collections::HashSet;

pub mod velocity;
pub use velocity::{
    NarrativeShift, SentimentDataPoint, SentimentDynamics, SentimentVelocityCalculator,
    VelocitySignal,
};

const NEGATION_WORDS: &[&str] = &[
    "not", "no", "never", "don't", "doesn't", "didn't", "isn't", "aren't",
    "wasn't", "weren't", "won't", "wouldn't", "couldn't", "shouldn't", "hardly",
    "barely", "neither", "nor", "without",
];

const NEGATION_WINDOW: usize = 3;

/// News event type with importance weight for signal generation
#[derive(Debug, Clone, Copy)]
enum NewsEventType {
    Earnings,       // Earnings reports, guidance
    MergersAcq,     // M&A, buyouts, spinoffs
    Regulatory,     // FDA, SEC, antitrust
    AnalystAction,  // Upgrades, downgrades, initiations
    Management,     // CEO changes, board reshuffles
    Product,        // Product launches, recalls
    Legal,          // Lawsuits, settlements
    Macro,          // Fed, economic data
    General,        // Catch-all
}

impl NewsEventType {
    fn importance_weight(&self) -> f64 {
        match self {
            NewsEventType::Earnings => 2.0,
            NewsEventType::MergersAcq => 2.5,
            NewsEventType::Regulatory => 2.0,
            NewsEventType::AnalystAction => 1.5,
            NewsEventType::Management => 1.3,
            NewsEventType::Product => 1.2,
            NewsEventType::Legal => 1.5,
            NewsEventType::Macro => 0.8,
            NewsEventType::General => 1.0,
        }
    }
}

fn classify_event(title: &str, description: Option<&str>) -> NewsEventType {
    let text = format!("{} {}", title, description.unwrap_or("")).to_lowercase();

    if text.contains("earnings") || text.contains("quarterly") || text.contains("guidance")
        || text.contains("revenue") && (text.contains("beat") || text.contains("miss") || text.contains("report"))
        || text.contains("eps") || text.contains("profit") && text.contains("quarter") {
        NewsEventType::Earnings
    } else if text.contains("acqui") || text.contains("merger") || text.contains("buyout")
        || text.contains("takeover") || text.contains("spinoff") || text.contains("spin-off") {
        NewsEventType::MergersAcq
    } else if text.contains("fda") || text.contains("sec ") || text.contains("regulat")
        || text.contains("approval") || text.contains("antitrust") || text.contains("compliance") {
        NewsEventType::Regulatory
    } else if text.contains("upgrade") || text.contains("downgrade") || text.contains("price target")
        || text.contains("initiat") || text.contains("analyst") || text.contains("rating") {
        NewsEventType::AnalystAction
    } else if text.contains("ceo") || text.contains("cfo") || text.contains("board")
        || text.contains("executive") || text.contains("resign") || text.contains("appoint") {
        NewsEventType::Management
    } else if text.contains("launch") || text.contains("product") || text.contains("recall")
        || text.contains("patent") || text.contains("innovation") {
        NewsEventType::Product
    } else if text.contains("lawsuit") || text.contains("litigation") || text.contains("settlement")
        || text.contains("sued") || text.contains("court") || text.contains("indictment") {
        NewsEventType::Legal
    } else if text.contains("fed ") || text.contains("federal reserve") || text.contains("interest rate")
        || text.contains("inflation") || text.contains("gdp") || text.contains("unemployment") {
        NewsEventType::Macro
    } else {
        NewsEventType::General
    }
}

pub struct SentimentAnalysisEngine {
    positive_words: Vec<&'static str>,
    negative_words: Vec<&'static str>,
    /// Optional FinBERT ML client for NLP-based sentiment
    finbert_client: Option<ml_client::SentimentClient>,
}

impl SentimentAnalysisEngine {
    pub fn new() -> Self {
        // Try to create FinBERT client from env
        let finbert_client = std::env::var("ML_SENTIMENT_URL")
            .ok()
            .or_else(|| Some("http://localhost:8003".to_string()))
            .map(|url| ml_client::SentimentClient::new(url, std::time::Duration::from_secs(5)));

        Self {
            positive_words: vec![
                "bullish", "rally", "surge", "gain", "profit", "growth", "beat",
                "upgrade", "outperform", "strong", "positive", "rise", "increase",
                "breakthrough", "innovation", "success", "exceed", "momentum",
                "buy", "recommend", "optimistic", "record", "high", "advance",
                // Financial-specific terms
                "dividend", "buyback", "repurchase", "accretive", "upside",
                "recovery", "rebound", "expansion", "robust", "accelerating",
                "overweight", "raised", "guidance", "upgraded", "initiated",
                "reiterated", "outpacing", "tailwind",
            ],
            negative_words: vec![
                "bearish", "decline", "loss", "fall", "plunge", "crash", "miss",
                "downgrade", "underperform", "weak", "negative", "drop", "decrease",
                "concern", "risk", "fail", "disappoint", "slump", "sell",
                "warning", "pessimistic", "low", "retreat", "fear", "trouble",
                // Financial-specific terms
                "dilution", "dilutive", "headwind", "lawsuit", "litigation",
                "recall", "investigation", "probe", "default", "bankruptcy",
                "restructuring", "layoff", "downside", "overvalued", "bubble",
                "underweight", "lowered", "suspended",
            ],
            finbert_client,
        }
    }

    fn analyze_text(&self, text: &str) -> f64 {
        let text_lower = text.to_lowercase();
        // Split into words, stripping common punctuation
        let words: Vec<&str> = text_lower
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '.' || c == '!' || c == '?')
            .filter(|w| !w.is_empty())
            .collect();

        let positive_set: HashSet<&str> = self.positive_words.iter().copied().collect();
        let negative_set: HashSet<&str> = self.negative_words.iter().copied().collect();
        let negation_set: HashSet<&str> = NEGATION_WORDS.iter().copied().collect();

        // Track positions of negation words
        let negation_positions: Vec<usize> = words
            .iter()
            .enumerate()
            .filter(|(_, w)| negation_set.contains(*w))
            .map(|(i, _)| i)
            .collect();

        let mut score: i32 = 0;

        for (i, word) in words.iter().enumerate() {
            let is_positive = positive_set.contains(*word);
            let is_negative = negative_set.contains(*word);

            if !is_positive && !is_negative {
                continue;
            }

            // Check if any negation word is within NEGATION_WINDOW before this word
            let negated = negation_positions.iter().any(|&neg_pos| {
                neg_pos < i && (i - neg_pos) <= NEGATION_WINDOW
            });

            if is_positive {
                score += if negated { -1 } else { 1 };
            } else {
                score += if negated { 1 } else { -1 };
            }
        }

        score as f64
    }

    fn analyze_article(&self, article: &NewsArticle) -> f64 {
        let mut total_score = 0.0;

        // Analyze title (weight it more heavily)
        total_score += self.analyze_text(&article.title) * 2.0;

        // Analyze description if available
        if let Some(desc) = &article.description {
            total_score += self.analyze_text(desc);
        }

        // Analyze keywords
        for keyword in &article.keywords {
            total_score += self.analyze_text(keyword) * 0.5;
        }

        total_score
    }

    #[allow(dead_code)]
    fn calculate_recency_weight(&self, article: &NewsArticle) -> f64 {
        let now = Utc::now();
        let age_hours = (now - article.published_utc).num_hours();

        // Exponential decay: news older than 24 hours gets less weight
        if age_hours < 0 {
            1.0
        } else if age_hours < 24 {
            1.0
        } else if age_hours < 48 {
            0.7
        } else if age_hours < 168 {
            // 1 week
            0.4
        } else {
            0.2
        }
    }

    /// Calculate entity relevance weight — articles that directly mention the
    /// target symbol in their tickers list are weighted more heavily.
    fn calculate_entity_weight(&self, article: &NewsArticle, symbol: &str) -> f64 {
        let sym_upper = symbol.to_uppercase();
        let is_primary = article.tickers.iter().any(|t| t.to_uppercase() == sym_upper);

        if is_primary {
            // Check if it's the *only* ticker (highly relevant) or one of many
            if article.tickers.len() <= 2 {
                1.5 // Focused article about this stock
            } else {
                1.2 // Mentioned alongside several others
            }
        } else {
            0.5 // Symbol not in tickers — peripheral mention at best
        }
    }

    /// Try FinBERT NLP analysis on article titles. Returns per-article scores.
    async fn try_finbert_scores(&self, news: &[NewsArticle]) -> Option<Vec<f64>> {
        let client = self.finbert_client.as_ref()?;
        let titles: Vec<String> = news.iter().map(|a| a.title.clone()).collect();
        match client.predict(titles, None).await {
            Ok(response) => {
                let scores: Vec<f64> = response.predictions.iter().map(|p| {
                    p.score * if p.label == "positive" { 1.0 } else if p.label == "negative" { -1.0 } else { 0.0 }
                }).collect();
                tracing::info!("FinBERT scored {} articles", scores.len());
                Some(scores)
            }
            Err(e) => {
                tracing::debug!("FinBERT unavailable, falling back to word-list: {}", e);
                None
            }
        }
    }

    /// Detect abnormal news buzz (more articles than usual suggests attention spike)
    #[allow(dead_code)]
    fn detect_buzz(&self, news: &[NewsArticle]) -> (f64, bool) {
        if news.is_empty() {
            return (0.0, false);
        }
        let now = Utc::now();
        let last_24h = news.iter().filter(|a| (now - a.published_utc).num_hours() < 24).count();
        let last_48h = news.iter().filter(|a| (now - a.published_utc).num_hours() < 48).count();
        let older = news.len().saturating_sub(last_48h);

        // Buzz ratio: articles in last 24h vs expected daily rate
        let expected_daily = if older > 0 {
            // Approximate daily rate from older articles
            let older_days = news.iter()
                .filter(|a| (now - a.published_utc).num_hours() >= 48)
                .map(|a| (now - a.published_utc).num_days())
                .max()
                .unwrap_or(7) as f64;
            if older_days > 0.0 { older as f64 / older_days } else { 1.0 }
        } else {
            1.0
        };

        let buzz_ratio = last_24h as f64 / expected_daily.max(0.5);
        let is_abnormal = buzz_ratio > 2.5 && last_24h >= 3;
        (buzz_ratio, is_abnormal)
    }

    async fn analyze_full(
        &self,
        symbol: &str,
        news: &[NewsArticle],
    ) -> Result<AnalysisResult, AnalysisError> {
        if news.is_empty() {
            return Ok(AnalysisResult {
                symbol: symbol.to_string(),
                timestamp: Utc::now(),
                signal: SignalStrength::Neutral,
                confidence: 0.0,
                reason: "No news articles available".to_string(),
                metrics: json!({}),
            });
        }

        // Try FinBERT first, fall back to word-list
        let finbert_scores = self.try_finbert_scores(news).await;
        let using_finbert = finbert_scores.is_some();

        // FIRST PASS: Compute raw sentiment scores for all articles
        let mut article_scores: Vec<f64> = Vec::with_capacity(news.len());
        for (i, article) in news.iter().enumerate() {
            // Sentiment score: prefer FinBERT, fall back to word-list
            let sentiment_score = if let Some(ref scores) = finbert_scores {
                scores.get(i).copied().unwrap_or(0.0) * 3.0 // Scale FinBERT [-1,1] to match word-list range
            } else {
                self.analyze_article(article)
            };
            article_scores.push(sentiment_score);
        }

        // Compute adaptive recency decay based on data span
        let now = Utc::now();
        let max_age_hours = news.iter()
            .map(|a| (now - a.published_utc).num_hours() as f64)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(24.0)
            .max(0.0);

        // Half-life is 1/4 of data span, clamped to reasonable range
        let half_life_hours = (max_age_hours / 4.0).clamp(12.0, 168.0);

        // SECOND PASS: Weighted aggregation with adaptive classification
        let mut total_score = 0.0;
        let mut total_weight = 0.0;
        let mut positive_count = 0;
        let mut negative_count = 0;
        let mut neutral_count = 0;
        let mut direct_mention_count = 0;
        let mut event_counts = std::collections::HashMap::new();

        for (i, article) in news.iter().enumerate() {
            let sentiment_score = article_scores[i];

            // Adaptive recency weight using continuous exponential decay
            let age_hours = (now - article.published_utc).num_hours() as f64;
            let recency_weight = if age_hours < 0.0 {
                1.0
            } else {
                (0.5_f64).powf(age_hours / half_life_hours)
            };

            let entity_weight = self.calculate_entity_weight(article, symbol);

            // Event classification: weight by importance
            let event_type = classify_event(&article.title, article.description.as_deref());
            let event_weight = event_type.importance_weight();
            *event_counts.entry(format!("{:?}", event_type)).or_insert(0u32) += 1;

            let combined_weight = recency_weight * entity_weight * event_weight;
            total_score += sentiment_score * combined_weight;
            total_weight += combined_weight;

            if entity_weight >= 1.0 {
                direct_mention_count += 1;
            }

            // Adaptive classification: use percentile ranks
            let percentile = adaptive::percentile_rank(sentiment_score, &article_scores);
            if percentile > 0.75 {
                positive_count += 1;
            } else if percentile < 0.25 {
                negative_count += 1;
            } else {
                neutral_count += 1;
            }
        }

        let avg_sentiment = if total_weight > 0.0 {
            total_score / total_weight
        } else {
            0.0
        };

        // Adaptive normalization using standard deviation
        let score_std = adaptive::std_dev(&article_scores);
        let normalized_score = if score_std > 0.0 {
            100.0 * (avg_sentiment / score_std).tanh()
        } else {
            100.0 * (avg_sentiment / 3.0).tanh()
        };

        let signal = SignalStrength::from_score(normalized_score as i32);

        // Adaptive buzz detection using z-score
        let last_24h = news.iter().filter(|a| (now - a.published_utc).num_hours() < 24).count();
        let last_48h = news.iter().filter(|a| (now - a.published_utc).num_hours() < 48).count();
        let older = news.len().saturating_sub(last_48h);

        let expected_daily = if older > 0 {
            let older_days = news.iter()
                .filter(|a| (now - a.published_utc).num_hours() >= 48)
                .map(|a| (now - a.published_utc).num_days())
                .max()
                .unwrap_or(7) as f64;
            if older_days > 0.0 { older as f64 / older_days } else { 1.0 }
        } else {
            1.0
        };

        let buzz_z = if expected_daily > 0.0 {
            (last_24h as f64 - expected_daily) / expected_daily.sqrt().max(1.0)
        } else {
            0.0
        };

        let buzz_ratio = last_24h as f64 / expected_daily.max(0.5);
        let abnormal_buzz = buzz_z > 2.0; // 2 standard deviations above expected

        // --- Sentiment Momentum (Acceleration/Deceleration) ---
        // Compare recent sentiment (last 24h) vs prior period (24-48h ago)
        let (sentiment_momentum, sentiment_acceleration) = if news.len() >= 2 {
            let last_24h_articles: Vec<&NewsArticle> = news.iter()
                .filter(|a| (now - a.published_utc).num_hours() < 24)
                .collect();
            let prev_24h_articles: Vec<&NewsArticle> = news.iter()
                .filter(|a| {
                    let hours = (now - a.published_utc).num_hours();
                    hours >= 24 && hours < 48
                })
                .collect();

            if !last_24h_articles.is_empty() && !prev_24h_articles.is_empty() {
                // Compute average sentiment for each period
                let recent_sent: f64 = last_24h_articles.iter()
                    .enumerate()
                    .map(|(i, _)| article_scores.get(i).copied().unwrap_or(0.0))
                    .sum::<f64>() / last_24h_articles.len() as f64;

                let prev_sent: f64 = prev_24h_articles.iter()
                    .enumerate()
                    .skip(last_24h_articles.len())
                    .take(prev_24h_articles.len())
                    .map(|(i, _)| article_scores.get(i).copied().unwrap_or(0.0))
                    .sum::<f64>() / prev_24h_articles.len() as f64;

                let momentum = recent_sent - prev_sent;
                let accel = if prev_sent != 0.0 { momentum / prev_sent.abs() } else { 0.0 };

                (Some(momentum), Some(accel))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // --- Contradictory Signal Detection ---
        // Identify if sentiment conflicts with price action or event type
        let contradictory_signal = if avg_sentiment.abs() > 1.0 {
            // Check if high-impact negative events (lawsuits, downgrades) coexist with positive sentiment
            let has_negative_event = event_counts.get("Legal").copied().unwrap_or(0) > 0
                || event_counts.get("Regulatory").copied().unwrap_or(0) > 0;
            let has_positive_event = event_counts.get("Earnings").copied().unwrap_or(0) > 0
                || event_counts.get("AnalystAction").copied().unwrap_or(0) > 0;

            if avg_sentiment > 2.0 && has_negative_event {
                Some("positive_sentiment_with_negative_events")
            } else if avg_sentiment < -2.0 && has_positive_event {
                Some("negative_sentiment_with_positive_events")
            } else {
                None
            }
        } else {
            None
        };

        // --- News Fatigue Detection ---
        // Multiple articles with neutral/declining sentiment = market desensitization
        let news_fatigue = if news.len() >= 10 && abnormal_buzz {
            // Check if sentiment is declining over time despite high volume
            if let Some(accel) = sentiment_acceleration {
                if accel < -0.2 && neutral_count > positive_count && neutral_count > negative_count {
                    Some(true)
                } else {
                    Some(false)
                }
            } else {
                None
            }
        } else {
            None
        };

        // Confidence: article count + consistency + FinBERT bonus
        let article_count_confidence = (news.len() as f64 / 10.0).min(1.0);
        let consistency = if !news.is_empty() {
            let max_count = positive_count.max(negative_count).max(neutral_count);
            max_count as f64 / news.len() as f64
        } else {
            0.0
        };
        let finbert_bonus = if using_finbert { 0.1 } else { 0.0 };
        let confidence = (article_count_confidence * 0.4 + consistency * 0.4 + finbert_bonus + 0.1).min(0.95);

        // Adaptive sentiment labels using z-score
        let sent_z = adaptive::z_score_of(avg_sentiment, &article_scores);
        let sentiment_label = if sent_z > 1.5 {
            "Very Positive"
        } else if sent_z > 0.5 {
            "Positive"
        } else if sent_z > -0.5 {
            "Neutral"
        } else if sent_z > -1.5 {
            "Negative"
        } else {
            "Very Negative"
        };

        // Generate signals for new features
        if let Some(mom) = sentiment_momentum {
            if mom > 1.5 {
                // Sentiment rapidly improving
            } else if mom < -1.5 {
                // Sentiment rapidly deteriorating
            }
        }

        if let Some(accel) = sentiment_acceleration {
            if accel > 0.3 {
                // Accelerating positive sentiment
            } else if accel < -0.3 {
                // Accelerating negative sentiment
            }
        }

        let buzz_label = if abnormal_buzz { " [HIGH BUZZ]" } else { "" };
        let fatigue_label = if news_fatigue == Some(true) { " [NEWS FATIGUE]" } else { "" };
        let contradiction_label = if contradictory_signal.is_some() { " [MIXED SIGNALS]" } else { "" };

        let reason = format!(
            "{} news sentiment ({} positive, {} negative, {} neutral){}{}{}",
            sentiment_label, positive_count, negative_count, neutral_count, buzz_label, fatigue_label, contradiction_label
        );

        let metrics = json!({
            "avg_sentiment": avg_sentiment,
            "normalized_score": normalized_score,
            "positive_articles": positive_count,
            "negative_articles": negative_count,
            "neutral_articles": neutral_count,
            "total_articles": news.len(),
            "direct_mention_articles": direct_mention_count,
            "using_finbert": using_finbert,
            "buzz_ratio": buzz_ratio,
            "buzz_z_score": buzz_z,
            "abnormal_buzz": abnormal_buzz,
            "sentiment_z_score": sent_z,
            "half_life_hours": half_life_hours,
            "event_breakdown": event_counts,
            "sentiment_momentum": sentiment_momentum,
            "sentiment_acceleration": sentiment_acceleration,
            "contradictory_signal": contradictory_signal,
            "news_fatigue": news_fatigue,
        });

        Ok(AnalysisResult {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            signal,
            confidence,
            reason,
            metrics,
        })
    }
}

#[async_trait]
impl SentimentAnalyzer for SentimentAnalysisEngine {
    async fn analyze(
        &self,
        symbol: &str,
        news: &[NewsArticle],
    ) -> Result<AnalysisResult, AnalysisError> {
        self.analyze_full(symbol, news).await
    }
}

impl Default for SentimentAnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}
