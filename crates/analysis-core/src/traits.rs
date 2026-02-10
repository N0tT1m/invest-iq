use async_trait::async_trait;
use crate::{AnalysisResult, Bar, Financials, NewsArticle, AnalysisError};

/// Trait for technical analysis engines
#[async_trait]
pub trait TechnicalAnalyzer: Send + Sync {
    async fn analyze(&self, symbol: &str, bars: &[Bar]) -> Result<AnalysisResult, AnalysisError>;
}

/// Trait for fundamental analysis engines
#[async_trait]
pub trait FundamentalAnalyzer: Send + Sync {
    async fn analyze(&self, symbol: &str, financials: &Financials) -> Result<AnalysisResult, AnalysisError>;
}

/// Trait for quantitative analysis engines
#[async_trait]
pub trait QuantAnalyzer: Send + Sync {
    async fn analyze(&self, symbol: &str, bars: &[Bar]) -> Result<AnalysisResult, AnalysisError>;
}

/// Trait for sentiment analysis engines
#[async_trait]
pub trait SentimentAnalyzer: Send + Sync {
    async fn analyze(&self, symbol: &str, news: &[NewsArticle]) -> Result<AnalysisResult, AnalysisError>;
}
