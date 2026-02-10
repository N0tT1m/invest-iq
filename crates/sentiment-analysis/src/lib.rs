use analysis_core::{AnalysisError, AnalysisResult, NewsArticle, SentimentAnalyzer, SignalStrength};
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

pub struct SentimentAnalysisEngine {
    positive_words: Vec<&'static str>,
    negative_words: Vec<&'static str>,
}

impl SentimentAnalysisEngine {
    pub fn new() -> Self {
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

    fn analyze_sync(
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

        let mut total_score = 0.0;
        let mut total_weight = 0.0;
        let mut positive_count = 0;
        let mut negative_count = 0;
        let mut neutral_count = 0;
        let mut direct_mention_count = 0;

        for article in news {
            let sentiment_score = self.analyze_article(article);
            let recency_weight = self.calculate_recency_weight(article);
            let entity_weight = self.calculate_entity_weight(article, symbol);

            let combined_weight = recency_weight * entity_weight;
            total_score += sentiment_score * combined_weight;
            total_weight += combined_weight;

            if entity_weight >= 1.0 {
                direct_mention_count += 1;
            }

            if sentiment_score > 0.5 {
                positive_count += 1;
            } else if sentiment_score < -0.5 {
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

        // Normalize to -100 to 100 scale using tanh for smooth mapping without hard clamping
        // avg_sentiment ±3 → ~±75, ±5 → ~±93, preserves granularity
        let normalized_score = 100.0 * (avg_sentiment / 3.0).tanh();

        let signal = SignalStrength::from_score(normalized_score as i32);

        // Confidence based on number of articles and consistency
        let article_count_confidence = (news.len() as f64 / 10.0).min(1.0);
        let consistency = if news.len() > 0 {
            let max_count = positive_count.max(negative_count).max(neutral_count);
            max_count as f64 / news.len() as f64
        } else {
            0.0
        };

        let confidence = (article_count_confidence * 0.5 + consistency * 0.5).min(0.9);

        let sentiment_label = if avg_sentiment > 1.0 {
            "Very Positive"
        } else if avg_sentiment > 0.3 {
            "Positive"
        } else if avg_sentiment > -0.3 {
            "Neutral"
        } else if avg_sentiment > -1.0 {
            "Negative"
        } else {
            "Very Negative"
        };

        let reason = format!(
            "{} news sentiment ({} positive, {} negative, {} neutral)",
            sentiment_label, positive_count, negative_count, neutral_count
        );

        let metrics = json!({
            "avg_sentiment": avg_sentiment,
            "normalized_score": normalized_score,
            "positive_articles": positive_count,
            "negative_articles": negative_count,
            "neutral_articles": neutral_count,
            "total_articles": news.len(),
            "direct_mention_articles": direct_mention_count,
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
        self.analyze_sync(symbol, news)
    }
}

impl Default for SentimentAnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}
