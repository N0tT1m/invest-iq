use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

const BASE_URL: &str = "https://www.alphavantage.co/query";

#[derive(Clone)]
pub struct AlphaVantageClient {
    api_key: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TechnicalIndicatorData {
    pub timestamp: String,
    pub value: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RSIData {
    pub timestamp: String,
    #[serde(rename = "RSI")]
    pub rsi: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MACDData {
    pub timestamp: String,
    #[serde(rename = "MACD")]
    pub macd: String,
    #[serde(rename = "MACD_Signal")]
    pub macd_signal: String,
    #[serde(rename = "MACD_Hist")]
    pub macd_hist: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SMAData {
    pub timestamp: String,
    #[serde(rename = "SMA")]
    pub sma: String,
}

impl AlphaVantageClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Get RSI data from Alpha Vantage
    pub async fn get_rsi(
        &self,
        symbol: &str,
        interval: &str,
        time_period: u32,
    ) -> Result<Vec<TechnicalIndicatorData>> {
        let url = format!(
            "{}?function=RSI&symbol={}&interval={}&time_period={}&apikey={}",
            BASE_URL, symbol, interval, time_period, self.api_key
        );

        let response = self.client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;

        // Check for error messages
        if let Some(error) = json.get("Error Message") {
            return Err(anyhow!("Alpha Vantage error: {}", error));
        }

        if let Some(note) = json.get("Note") {
            return Err(anyhow!("Alpha Vantage rate limit: {}", note));
        }

        // Parse RSI data
        let technical_analysis = json
            .get("Technical Analysis: RSI")
            .ok_or_else(|| anyhow!("No RSI data found"))?;

        let mut data = Vec::new();
        if let Some(obj) = technical_analysis.as_object() {
            for (timestamp, values) in obj {
                if let Some(rsi_str) = values.get("RSI").and_then(|v| v.as_str()) {
                    if let Ok(rsi) = rsi_str.parse::<f64>() {
                        data.push(TechnicalIndicatorData {
                            timestamp: timestamp.clone(),
                            value: rsi,
                        });
                    }
                }
            }
        }

        // Sort by timestamp descending (most recent first)
        data.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(data)
    }

    /// Get MACD data from Alpha Vantage
    pub async fn get_macd(&self, symbol: &str, interval: &str) -> Result<Vec<MACDIndicatorData>> {
        let url = format!(
            "{}?function=MACD&symbol={}&interval={}&series_type=close&apikey={}",
            BASE_URL, symbol, interval, self.api_key
        );

        let response = self.client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;

        if let Some(error) = json.get("Error Message") {
            return Err(anyhow!("Alpha Vantage error: {}", error));
        }

        if let Some(note) = json.get("Note") {
            return Err(anyhow!("Alpha Vantage rate limit: {}", note));
        }

        let technical_analysis = json
            .get("Technical Analysis: MACD")
            .ok_or_else(|| anyhow!("No MACD data found"))?;

        let mut data = Vec::new();
        if let Some(obj) = technical_analysis.as_object() {
            for (timestamp, values) in obj {
                let macd = values
                    .get("MACD")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok());
                let signal = values
                    .get("MACD_Signal")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok());
                let hist = values
                    .get("MACD_Hist")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok());

                if let (Some(macd), Some(signal), Some(hist)) = (macd, signal, hist) {
                    data.push(MACDIndicatorData {
                        timestamp: timestamp.clone(),
                        macd,
                        signal,
                        histogram: hist,
                    });
                }
            }
        }

        data.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(data)
    }

    /// Get SMA data from Alpha Vantage
    pub async fn get_sma(
        &self,
        symbol: &str,
        interval: &str,
        time_period: u32,
    ) -> Result<Vec<TechnicalIndicatorData>> {
        let url = format!(
            "{}?function=SMA&symbol={}&interval={}&time_period={}&series_type=close&apikey={}",
            BASE_URL, symbol, interval, time_period, self.api_key
        );

        let response = self.client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;

        if let Some(error) = json.get("Error Message") {
            return Err(anyhow!("Alpha Vantage error: {}", error));
        }

        if let Some(note) = json.get("Note") {
            return Err(anyhow!("Alpha Vantage rate limit: {}", note));
        }

        let technical_analysis = json
            .get("Technical Analysis: SMA")
            .ok_or_else(|| anyhow!("No SMA data found"))?;

        let mut data = Vec::new();
        if let Some(obj) = technical_analysis.as_object() {
            for (timestamp, values) in obj {
                if let Some(sma_str) = values.get("SMA").and_then(|v| v.as_str()) {
                    if let Ok(sma) = sma_str.parse::<f64>() {
                        data.push(TechnicalIndicatorData {
                            timestamp: timestamp.clone(),
                            value: sma,
                        });
                    }
                }
            }
        }

        data.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(data)
    }

    /// Get company fundamentals overview
    pub async fn get_company_overview(&self, symbol: &str) -> Result<CompanyOverview> {
        let url = format!(
            "{}?function=OVERVIEW&symbol={}&apikey={}",
            BASE_URL, symbol, self.api_key
        );

        let response = self.client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;

        if let Some(error) = json.get("Error Message") {
            return Err(anyhow!("Alpha Vantage error: {}", error));
        }

        let overview: CompanyOverview = serde_json::from_value(json)?;
        Ok(overview)
    }
}

#[derive(Debug, Clone)]
pub struct MACDIndicatorData {
    pub timestamp: String,
    pub macd: f64,
    pub signal: f64,
    pub histogram: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompanyOverview {
    #[serde(rename = "Symbol")]
    pub symbol: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "PERatio")]
    pub pe_ratio: Option<String>,
    #[serde(rename = "PEGRatio")]
    pub peg_ratio: Option<String>,
    #[serde(rename = "BookValue")]
    pub book_value: Option<String>,
    #[serde(rename = "DividendYield")]
    pub dividend_yield: Option<String>,
    #[serde(rename = "EPS")]
    pub eps: Option<String>,
    #[serde(rename = "ReturnOnEquityTTM")]
    pub roe: Option<String>,
    #[serde(rename = "ProfitMargin")]
    pub profit_margin: Option<String>,
    #[serde(rename = "Beta")]
    pub beta: Option<String>,
    #[serde(rename = "52WeekHigh")]
    pub week_52_high: Option<String>,
    #[serde(rename = "52WeekLow")]
    pub week_52_low: Option<String>,
}
