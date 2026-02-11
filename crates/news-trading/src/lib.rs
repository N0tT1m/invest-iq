use analysis_core::NewsArticle;
use anyhow::Result;
use chrono::{DateTime, Utc, Duration};
use polygon_client::PolygonClient;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use log::warn;

/// News sentiment classification
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NewsSentiment {
    /// Strongly positive (bullish)
    VeryPositive,

    /// Moderately positive
    Positive,

    /// Neutral
    Neutral,

    /// Moderately negative
    Negative,

    /// Strongly negative (bearish)
    VeryNegative,
}

impl NewsSentiment {
    /// Convert sentiment to numeric score (-1.0 to 1.0)
    pub fn to_score(&self) -> f64 {
        match self {
            NewsSentiment::VeryPositive => 1.0,
            NewsSentiment::Positive => 0.5,
            NewsSentiment::Neutral => 0.0,
            NewsSentiment::Negative => -0.5,
            NewsSentiment::VeryNegative => -1.0,
        }
    }

    /// Create from numeric score
    pub fn from_score(score: f64) -> Self {
        if score >= 0.6 {
            NewsSentiment::VeryPositive
        } else if score >= 0.2 {
            NewsSentiment::Positive
        } else if score >= -0.2 {
            NewsSentiment::Neutral
        } else if score >= -0.6 {
            NewsSentiment::Negative
        } else {
            NewsSentiment::VeryNegative
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            NewsSentiment::VeryPositive => "Very Positive",
            NewsSentiment::Positive => "Positive",
            NewsSentiment::Neutral => "Neutral",
            NewsSentiment::Negative => "Negative",
            NewsSentiment::VeryNegative => "Very Negative",
        }
    }
}

/// News analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsAnalysis {
    pub article: NewsArticle,
    pub sentiment: NewsSentiment,
    pub confidence: f64,
    pub impact_score: f64,  // 0.0 to 1.0, higher = more impactful
    pub analyzed_at: DateTime<Utc>,
}

/// Aggregated news sentiment for a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedNewsSentiment {
    pub symbol: String,
    pub overall_sentiment: NewsSentiment,
    pub sentiment_score: f64,
    pub article_count: usize,
    pub recent_articles: Vec<NewsAnalysis>,
    pub time_window_hours: i64,
    pub analyzed_at: DateTime<Utc>,
}

/// News trading signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsSignal {
    pub symbol: String,
    pub signal_type: NewsSignalType,
    pub sentiment: AggregatedNewsSentiment,
    pub urgency: Urgency,
    pub reasoning: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NewsSignalType {
    Buy,
    Sell,
    StrongBuy,
    StrongSell,
    NoAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Urgency {
    /// Act within seconds (breaking news)
    Immediate,

    /// Act within minutes
    High,

    /// Act within hours
    Medium,

    /// No rush
    Low,
}

/// News scanner and analyzer
pub struct NewsScanner {
    polygon_client: PolygonClient,
    sentiment_service_url: Option<String>,
    http_client: Client,
    #[allow(dead_code)]
    scan_interval_seconds: u64,
}

impl NewsScanner {
    pub fn new(polygon_api_key: String) -> Self {
        Self {
            polygon_client: PolygonClient::new(polygon_api_key),
            sentiment_service_url: None,
            http_client: Client::new(),
            scan_interval_seconds: 60,  // Check every minute
        }
    }

    pub fn with_sentiment_service(polygon_api_key: String, service_url: String) -> Self {
        Self {
            polygon_client: PolygonClient::new(polygon_api_key),
            sentiment_service_url: Some(service_url),
            http_client: Client::new(),
            scan_interval_seconds: 60,
        }
    }

    /// Scan for recent news articles from both Polygon and Finnhub
    pub async fn scan_news(&self, symbol: Option<&str>, limit: u32) -> Result<Vec<NewsArticle>> {
        // Fetch from both sources concurrently
        let polygon_future = self.polygon_client.get_news(symbol, limit);
        let finnhub_future = async {
            if let Some(sym) = symbol {
                self.polygon_client.get_finnhub_news(sym, 7).await.unwrap_or_default()
            } else {
                // For general news, try Finnhub general news endpoint
                self.polygon_client.get_finnhub_general_news().await.unwrap_or_default()
            }
        };

        let (polygon_result, finnhub_articles) = tokio::join!(polygon_future, finnhub_future);

        let mut articles = polygon_result
            .map_err(|e| anyhow::anyhow!("Failed to fetch news: {}", e))?;

        // Merge Finnhub articles, deduplicate by title similarity
        for fa in finnhub_articles {
            // Simple deduplication: check if any existing article has a similar title
            let is_duplicate = articles.iter().any(|a| {
                let a_title_lower = a.title.to_lowercase();
                let fa_title_lower = fa.title.to_lowercase();
                let a_prefix: String = a_title_lower.chars().take(30).collect();
                let fa_prefix: String = fa_title_lower.chars().take(30).collect();

                a_title_lower.contains(&fa_prefix) || fa_title_lower.contains(&a_prefix)
            });

            if !is_duplicate {
                articles.push(fa);
            }
        }

        // Sort by published date descending, limit
        articles.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));
        articles.truncate(limit as usize);

        Ok(articles)
    }

    /// Analyze sentiment of a single article
    pub async fn analyze_article(&self, article: &NewsArticle) -> Result<NewsAnalysis> {
        let sentiment_score = if let Some(url) = &self.sentiment_service_url {
            // Use FinBERT service
            match self.query_finbert(url, article).await {
                Ok(score) => score,
                Err(e) => {
                    warn!("FinBERT service failed: {}. Using keyword-based fallback.", e);
                    self.keyword_based_sentiment(article)
                }
            }
        } else {
            // Fallback to keyword-based sentiment
            self.keyword_based_sentiment(article)
        };

        let sentiment = NewsSentiment::from_score(sentiment_score);
        let confidence = sentiment_score.abs().min(1.0);
        let impact_score = self.calculate_impact_score(article);

        Ok(NewsAnalysis {
            article: article.clone(),
            sentiment,
            confidence,
            impact_score,
            analyzed_at: Utc::now(),
        })
    }

    /// Analyze multiple articles and aggregate sentiment
    pub async fn analyze_aggregated(
        &self,
        symbol: &str,
        time_window_hours: i64,
    ) -> Result<AggregatedNewsSentiment> {
        // Fetch recent news
        let articles = self.scan_news(Some(symbol), 50).await?;

        // Filter by time window
        let cutoff = Utc::now() - Duration::hours(time_window_hours);
        let recent_articles: Vec<_> = articles.into_iter()
            .filter(|a| a.published_utc >= cutoff)
            .collect();

        if recent_articles.is_empty() {
            return Ok(AggregatedNewsSentiment {
                symbol: symbol.to_string(),
                overall_sentiment: NewsSentiment::Neutral,
                sentiment_score: 0.0,
                article_count: 0,
                recent_articles: vec![],
                time_window_hours,
                analyzed_at: Utc::now(),
            });
        }

        // Analyze each article
        let mut analyzed_articles = Vec::new();
        for article in recent_articles {
            if let Ok(analysis) = self.analyze_article(&article).await {
                analyzed_articles.push(analysis);
            }
        }

        // Calculate weighted average sentiment
        let total_weight: f64 = analyzed_articles.iter()
            .map(|a| a.impact_score * a.confidence)
            .sum();

        let weighted_sentiment: f64 = analyzed_articles.iter()
            .map(|a| a.sentiment.to_score() * a.impact_score * a.confidence)
            .sum();

        let sentiment_score = if total_weight > 0.0 {
            weighted_sentiment / total_weight
        } else {
            0.0
        };

        let overall_sentiment = NewsSentiment::from_score(sentiment_score);

        Ok(AggregatedNewsSentiment {
            symbol: symbol.to_string(),
            overall_sentiment,
            sentiment_score,
            article_count: analyzed_articles.len(),
            recent_articles: analyzed_articles,
            time_window_hours,
            analyzed_at: Utc::now(),
        })
    }

    /// Generate trading signal from news sentiment
    pub fn generate_signal(
        &self,
        aggregated: AggregatedNewsSentiment,
    ) -> NewsSignal {
        let (signal_type, urgency, confidence, reasoning) = match aggregated.overall_sentiment {
            NewsSentiment::VeryPositive => {
                if aggregated.article_count >= 3 {
                    (
                        NewsSignalType::StrongBuy,
                        Urgency::Immediate,
                        0.9,
                        format!("Very positive news ({} articles, score: {:.2})",
                            aggregated.article_count, aggregated.sentiment_score)
                    )
                } else {
                    (
                        NewsSignalType::Buy,
                        Urgency::High,
                        0.7,
                        format!("Positive news (score: {:.2})", aggregated.sentiment_score)
                    )
                }
            }
            NewsSentiment::Positive => {
                (
                    NewsSignalType::Buy,
                    Urgency::Medium,
                    0.6,
                    format!("Moderately positive news (score: {:.2})", aggregated.sentiment_score)
                )
            }
            NewsSentiment::VeryNegative => {
                if aggregated.article_count >= 3 {
                    (
                        NewsSignalType::StrongSell,
                        Urgency::Immediate,
                        0.9,
                        format!("Very negative news ({} articles, score: {:.2})",
                            aggregated.article_count, aggregated.sentiment_score)
                    )
                } else {
                    (
                        NewsSignalType::Sell,
                        Urgency::High,
                        0.7,
                        format!("Negative news (score: {:.2})", aggregated.sentiment_score)
                    )
                }
            }
            NewsSentiment::Negative => {
                (
                    NewsSignalType::Sell,
                    Urgency::Medium,
                    0.6,
                    format!("Moderately negative news (score: {:.2})", aggregated.sentiment_score)
                )
            }
            NewsSentiment::Neutral => {
                (
                    NewsSignalType::NoAction,
                    Urgency::Low,
                    0.5,
                    "Neutral news sentiment".to_string()
                )
            }
        };

        NewsSignal {
            symbol: aggregated.symbol.clone(),
            signal_type,
            sentiment: aggregated,
            urgency,
            reasoning,
            confidence,
        }
    }

    /// Query FinBERT sentiment service
    async fn query_finbert(&self, url: &str, article: &NewsArticle) -> Result<f64> {
        #[derive(Serialize)]
        struct SentimentRequest {
            text: String,
        }

        #[derive(Deserialize)]
        struct SentimentResponse {
            #[allow(dead_code)]
            sentiment: String,
            score: f64,
        }

        let text = format!(
            "{} {}",
            article.title,
            article.description.as_ref().unwrap_or(&String::new())
        );

        let request = SentimentRequest { text };

        let response = self.http_client
            .post(format!("{}/analyze_sentiment", url))
            .json(&request)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        let result: SentimentResponse = response.json().await?;

        Ok(result.score)
    }

    /// Keyword-based sentiment analysis (fallback)
    fn keyword_based_sentiment(&self, article: &NewsArticle) -> f64 {
        let positive_keywords = vec![
            "surges", "rally", "gains", "profit", "growth", "beats",
            "exceeds", "strong", "bullish", "upgrade", "optimistic",
            "breakthrough", "success", "record", "high", "soars",
        ];

        let negative_keywords = vec![
            "falls", "plunges", "losses", "decline", "weak", "misses",
            "cuts", "drops", "bearish", "downgrade", "pessimistic",
            "failure", "concern", "warning", "low", "crashes",
        ];

        let text = format!(
            "{} {}",
            article.title.to_lowercase(),
            article.description.as_ref().unwrap_or(&String::new()).to_lowercase()
        );

        let positive_count: i32 = positive_keywords.iter()
            .map(|kw| text.matches(kw).count() as i32)
            .sum();

        let negative_count: i32 = negative_keywords.iter()
            .map(|kw| text.matches(kw).count() as i32)
            .sum();

        let total = positive_count + negative_count;
        if total == 0 {
            return 0.0;
        }

        let score = (positive_count - negative_count) as f64 / total as f64;
        score.clamp(-1.0, 1.0)
    }

    /// Calculate impact score based on article metadata
    fn calculate_impact_score(&self, article: &NewsArticle) -> f64 {
        let mut score: f64 = 0.5;

        // Recency bonus (higher score for more recent articles)
        let age = Utc::now() - article.published_utc;
        if age < Duration::hours(1) {
            score += 0.3;
        } else if age < Duration::hours(6) {
            score += 0.2;
        } else if age < Duration::hours(24) {
            score += 0.1;
        }

        // Author reputation (simplified)
        if let Some(author) = &article.author {
            let reputable_sources = vec!["Reuters", "Bloomberg", "CNBC", "WSJ", "Financial Times"];
            if reputable_sources.iter().any(|s| author.contains(s)) {
                score += 0.2;
            }
        }

        score.min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentiment_score_conversion() {
        assert_eq!(NewsSentiment::from_score(0.8).to_score(), 1.0);
        assert_eq!(NewsSentiment::from_score(0.3).to_score(), 0.5);
        assert_eq!(NewsSentiment::from_score(0.0).to_score(), 0.0);
        assert_eq!(NewsSentiment::from_score(-0.3).to_score(), -0.5);
        assert_eq!(NewsSentiment::from_score(-0.8).to_score(), -1.0);
    }

    #[test]
    fn test_keyword_sentiment() {
        let scanner = NewsScanner::new("test_key".to_string());

        let positive_article = NewsArticle {
            id: "1".to_string(),
            title: "Stock surges on strong earnings beat".to_string(),
            author: None,
            published_utc: Utc::now(),
            article_url: "http://example.com".to_string(),
            description: Some("Company reports record profits and growth".to_string()),
            keywords: vec![],
            tickers: vec!["AAPL".to_string()],
        };

        let score = scanner.keyword_based_sentiment(&positive_article);
        assert!(score > 0.0);

        let negative_article = NewsArticle {
            id: "2".to_string(),
            title: "Stock plunges on weak guidance".to_string(),
            author: None,
            published_utc: Utc::now(),
            article_url: "http://example.com".to_string(),
            description: Some("Company warns of declining revenues".to_string()),
            keywords: vec![],
            tickers: vec!["TSLA".to_string()],
        };

        let score = scanner.keyword_based_sentiment(&negative_article);
        assert!(score < 0.0);
    }
}
