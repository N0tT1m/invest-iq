use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const BASE_URL: &str = "https://query2.finance.yahoo.com/v8/finance";
const CHART_URL: &str = "https://query2.finance.yahoo.com/v8/finance/chart";

#[derive(Clone)]
pub struct YahooFinanceClient {
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct YahooQuote {
    pub symbol: String,
    pub price: f64,
    pub change: f64,
    pub change_percent: f64,
    pub volume: Option<u64>,
    pub market_cap: Option<f64>,
    pub pe_ratio: Option<f64>,
    pub dividend_yield: Option<f64>,
    pub beta: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct YahooFundamentals {
    pub symbol: String,
    pub market_cap: Option<f64>,
    pub pe_ratio: Option<f64>,
    pub forward_pe: Option<f64>,
    pub peg_ratio: Option<f64>,
    pub price_to_book: Option<f64>,
    pub enterprise_value: Option<f64>,
    pub profit_margin: Option<f64>,
    pub operating_margin: Option<f64>,
    pub return_on_assets: Option<f64>,
    pub return_on_equity: Option<f64>,
    pub revenue: Option<f64>,
    pub revenue_per_share: Option<f64>,
    pub quarterly_revenue_growth: Option<f64>,
    pub gross_profit: Option<f64>,
    pub ebitda: Option<f64>,
    pub diluted_eps: Option<f64>,
    pub quarterly_earnings_growth: Option<f64>,
    pub total_cash: Option<f64>,
    pub total_debt: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub current_ratio: Option<f64>,
    pub book_value_per_share: Option<f64>,
    pub operating_cash_flow: Option<f64>,
    pub levered_free_cash_flow: Option<f64>,
    pub beta: Option<f64>,
    pub fifty_two_week_high: Option<f64>,
    pub fifty_two_week_low: Option<f64>,
    pub fifty_day_average: Option<f64>,
    pub two_hundred_day_average: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct YahooHistoricalData {
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

impl YahooFinanceClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .build()
                .unwrap(),
        }
    }

    /// Get quote data for a symbol
    pub async fn get_quote(&self, symbol: &str) -> Result<YahooQuote> {
        let url = format!("{}/quote?symbols={}", BASE_URL, symbol);

        let response = self.client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;

        // Parse the response
        let quote_response = json.get("quoteResponse")
            .and_then(|v| v.get("result"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| anyhow!("No quote data found for {}", symbol))?;

        let price = quote_response.get("regularMarketPrice")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let change = quote_response.get("regularMarketChange")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let change_percent = quote_response.get("regularMarketChangePercent")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        Ok(YahooQuote {
            symbol: symbol.to_string(),
            price,
            change,
            change_percent,
            volume: quote_response.get("regularMarketVolume").and_then(|v| v.as_u64()),
            market_cap: quote_response.get("marketCap").and_then(|v| v.as_f64()),
            pe_ratio: quote_response.get("trailingPE").and_then(|v| v.as_f64()),
            dividend_yield: quote_response.get("dividendYield").and_then(|v| v.as_f64()),
            beta: quote_response.get("beta").and_then(|v| v.as_f64()),
        })
    }

    /// Get detailed fundamental data
    pub async fn get_fundamentals(&self, symbol: &str) -> Result<YahooFundamentals> {
        let url = format!("{}/quote?symbols={}", BASE_URL, symbol);

        let response = self.client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;

        let data = json.get("quoteResponse")
            .and_then(|v| v.get("result"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| anyhow!("No fundamental data found for {}", symbol))?;

        Ok(YahooFundamentals {
            symbol: symbol.to_string(),
            market_cap: data.get("marketCap").and_then(|v| v.as_f64()),
            pe_ratio: data.get("trailingPE").and_then(|v| v.as_f64()),
            forward_pe: data.get("forwardPE").and_then(|v| v.as_f64()),
            peg_ratio: data.get("pegRatio").and_then(|v| v.as_f64()),
            price_to_book: data.get("priceToBook").and_then(|v| v.as_f64()),
            enterprise_value: data.get("enterpriseValue").and_then(|v| v.as_f64()),
            profit_margin: data.get("profitMargins").and_then(|v| v.as_f64()),
            operating_margin: data.get("operatingMargins").and_then(|v| v.as_f64()),
            return_on_assets: data.get("returnOnAssets").and_then(|v| v.as_f64()),
            return_on_equity: data.get("returnOnEquity").and_then(|v| v.as_f64()),
            revenue: data.get("totalRevenue").and_then(|v| v.as_f64()),
            revenue_per_share: data.get("revenuePerShare").and_then(|v| v.as_f64()),
            quarterly_revenue_growth: data.get("revenueGrowth").and_then(|v| v.as_f64()),
            gross_profit: data.get("grossProfits").and_then(|v| v.as_f64()),
            ebitda: data.get("ebitda").and_then(|v| v.as_f64()),
            diluted_eps: data.get("trailingEps").and_then(|v| v.as_f64()),
            quarterly_earnings_growth: data.get("earningsGrowth").and_then(|v| v.as_f64()),
            total_cash: data.get("totalCash").and_then(|v| v.as_f64()),
            total_debt: data.get("totalDebt").and_then(|v| v.as_f64()),
            debt_to_equity: data.get("debtToEquity").and_then(|v| v.as_f64()),
            current_ratio: data.get("currentRatio").and_then(|v| v.as_f64()),
            book_value_per_share: data.get("bookValue").and_then(|v| v.as_f64()),
            operating_cash_flow: data.get("operatingCashflow").and_then(|v| v.as_f64()),
            levered_free_cash_flow: data.get("freeCashflow").and_then(|v| v.as_f64()),
            beta: data.get("beta").and_then(|v| v.as_f64()),
            fifty_two_week_high: data.get("fiftyTwoWeekHigh").and_then(|v| v.as_f64()),
            fifty_two_week_low: data.get("fiftyTwoWeekLow").and_then(|v| v.as_f64()),
            fifty_day_average: data.get("fiftyDayAverage").and_then(|v| v.as_f64()),
            two_hundred_day_average: data.get("twoHundredDayAverage").and_then(|v| v.as_f64()),
        })
    }

    /// Get historical price data
    pub async fn get_historical_data(
        &self,
        symbol: &str,
        period1: i64,  // Unix timestamp
        period2: i64,  // Unix timestamp
        interval: &str, // 1d, 1h, etc.
    ) -> Result<Vec<YahooHistoricalData>> {
        let url = format!(
            "{}/{}?period1={}&period2={}&interval={}",
            CHART_URL, symbol, period1, period2, interval
        );

        let response = self.client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;

        let chart = json.get("chart")
            .and_then(|v| v.get("result"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| anyhow!("No chart data found"))?;

        let timestamps = chart.get("timestamp")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("No timestamps found"))?;

        let quotes = chart.get("indicators")
            .and_then(|v| v.get("quote"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| anyhow!("No quote data found"))?;

        let opens = quotes.get("open")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("No open prices"))?;

        let highs = quotes.get("high")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("No high prices"))?;

        let lows = quotes.get("low")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("No low prices"))?;

        let closes = quotes.get("close")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("No close prices"))?;

        let volumes = quotes.get("volume")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("No volumes"))?;

        let mut historical_data = Vec::new();

        for i in 0..timestamps.len() {
            if let (Some(ts), Some(o), Some(h), Some(l), Some(c), Some(v)) = (
                timestamps[i].as_i64(),
                opens[i].as_f64(),
                highs[i].as_f64(),
                lows[i].as_f64(),
                closes[i].as_f64(),
                volumes[i].as_u64(),
            ) {
                historical_data.push(YahooHistoricalData {
                    timestamp: DateTime::from_timestamp(ts, 0)
                        .ok_or_else(|| anyhow!("Invalid timestamp"))?,
                    open: o,
                    high: h,
                    low: l,
                    close: c,
                    volume: v,
                });
            }
        }

        Ok(historical_data)
    }
}

impl Default for YahooFinanceClient {
    fn default() -> Self {
        Self::new()
    }
}
