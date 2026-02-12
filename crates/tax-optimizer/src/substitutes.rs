//! Substitute Security Finder
//!
//! Finds replacement securities for tax-loss harvesting that maintain
//! similar exposure without triggering wash sale rules.

use serde::{Deserialize, Serialize};

/// Type of substitute security
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubstituteType {
    /// ETF tracking similar index
    SectorETF,
    /// Competitor in same industry
    Competitor,
    /// Index fund with similar exposure
    IndexFund,
    /// Different share class
    ShareClass,
    /// Leveraged version
    Leveraged,
    /// Bond/fixed income alternative
    FixedIncome,
}

impl std::fmt::Display for SubstituteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubstituteType::SectorETF => write!(f, "Sector ETF"),
            SubstituteType::Competitor => write!(f, "Competitor"),
            SubstituteType::IndexFund => write!(f, "Index Fund"),
            SubstituteType::ShareClass => write!(f, "Share Class"),
            SubstituteType::Leveraged => write!(f, "Leveraged"),
            SubstituteType::FixedIncome => write!(f, "Fixed Income"),
        }
    }
}

/// Correlation score between two securities
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CorrelationScore {
    /// Pearson correlation coefficient (-1 to 1)
    pub correlation: f64,
    /// How similar the beta is (0-1)
    pub beta_similarity: f64,
    /// Sector overlap (0-1)
    pub sector_overlap: f64,
    /// Overall score (0-1)
    pub overall: f64,
}

impl CorrelationScore {
    /// Calculate overall score from components
    pub fn calculate(correlation: f64, beta_similarity: f64, sector_overlap: f64) -> Self {
        let overall = correlation * 0.5 + beta_similarity * 0.3 + sector_overlap * 0.2;
        Self {
            correlation,
            beta_similarity,
            sector_overlap,
            overall,
        }
    }

    /// Create a score indicating identical security (would trigger wash sale)
    pub fn identical() -> Self {
        Self {
            correlation: 1.0,
            beta_similarity: 1.0,
            sector_overlap: 1.0,
            overall: 1.0,
        }
    }
}

/// A potential substitute security
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubstituteSecurity {
    /// Symbol
    pub symbol: String,
    /// Company/Fund name
    pub name: String,
    /// Type of substitute
    pub substitute_type: SubstituteType,
    /// Correlation with original
    pub correlation: CorrelationScore,
    /// Expense ratio (for ETFs/funds)
    pub expense_ratio: Option<f64>,
    /// Whether this is wash-sale safe
    pub wash_sale_safe: bool,
    /// Reason for recommendation
    pub reason: String,
    /// Risk level relative to original
    pub risk_comparison: String,
}

/// Configuration for substitute finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubstituteConfig {
    /// Minimum correlation to consider
    pub min_correlation: f64,
    /// Maximum expense ratio (for funds)
    pub max_expense_ratio: f64,
    /// Whether to include leveraged options
    pub include_leveraged: bool,
    /// Number of substitutes to return
    pub max_substitutes: usize,
}

impl Default for SubstituteConfig {
    fn default() -> Self {
        Self {
            min_correlation: 0.7,
            max_expense_ratio: 0.50, // 50 bps
            include_leveraged: false,
            max_substitutes: 5,
        }
    }
}

/// Engine for finding substitute securities
pub struct SubstituteFinder {
    config: SubstituteConfig,
    /// Pre-defined substitutes database
    substitutes_db: SubstitutesDatabase,
}

impl SubstituteFinder {
    /// Create a new substitute finder
    pub fn new() -> Self {
        Self {
            config: SubstituteConfig::default(),
            substitutes_db: SubstitutesDatabase::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: SubstituteConfig) -> Self {
        Self {
            config,
            substitutes_db: SubstitutesDatabase::default(),
        }
    }

    /// Find substitutes for a security
    pub fn find_substitutes(&self, symbol: &str) -> Vec<SubstituteSecurity> {
        let mut substitutes = Vec::new();

        // Check pre-defined substitutes first
        if let Some(predefined) = self.substitutes_db.get_substitutes(symbol) {
            substitutes.extend(predefined);
        }

        // Check sector ETFs
        if let Some(sector) = self.substitutes_db.get_sector(symbol) {
            if let Some(sector_etf) = self.substitutes_db.get_sector_etf(sector) {
                if sector_etf != symbol {
                    substitutes.push(SubstituteSecurity {
                        symbol: sector_etf.to_string(),
                        name: format!("{} Sector ETF", sector),
                        substitute_type: SubstituteType::SectorETF,
                        correlation: CorrelationScore::calculate(0.85, 0.80, 1.0),
                        expense_ratio: Some(0.10),
                        wash_sale_safe: true,
                        reason: format!("Broad {} sector exposure", sector),
                        risk_comparison: "Similar market risk, less single-stock risk".to_string(),
                    });
                }
            }
        }

        // Filter based on config
        substitutes.retain(|s| {
            s.correlation.overall >= self.config.min_correlation
                && s.expense_ratio
                    .is_none_or(|e| e <= self.config.max_expense_ratio)
                && (self.config.include_leveraged || s.substitute_type != SubstituteType::Leveraged)
        });

        // Sort by correlation
        substitutes.sort_by(|a, b| {
            b.correlation
                .overall
                .partial_cmp(&a.correlation.overall)
                .unwrap()
        });

        // Limit results
        substitutes.truncate(self.config.max_substitutes);

        substitutes
    }

    /// Check if two symbols are substantially identical (would trigger wash sale)
    pub fn is_substantially_identical(&self, symbol1: &str, symbol2: &str) -> bool {
        if symbol1 == symbol2 {
            return true;
        }

        // Check known identical pairs (e.g., different share classes)
        self.substitutes_db.are_identical(symbol1, symbol2)
    }

    /// Get the best substitute for a symbol
    pub fn best_substitute(&self, symbol: &str) -> Option<SubstituteSecurity> {
        self.find_substitutes(symbol).into_iter().next()
    }
}

impl Default for SubstituteFinder {
    fn default() -> Self {
        Self::new()
    }
}

/// Database of known substitutes and sector mappings
struct SubstitutesDatabase {
    /// Sector ETF mappings
    sector_etfs: std::collections::HashMap<String, String>,
    /// Stock to sector mapping
    stock_sectors: std::collections::HashMap<String, String>,
    /// Pre-defined substitute lists
    substitutes: std::collections::HashMap<String, Vec<SubstituteSecurity>>,
    /// Substantially identical pairs
    identical_pairs: Vec<(String, String)>,
}

impl Default for SubstitutesDatabase {
    fn default() -> Self {
        let mut sector_etfs = std::collections::HashMap::new();
        sector_etfs.insert("Technology".to_string(), "XLK".to_string());
        sector_etfs.insert("Financial".to_string(), "XLF".to_string());
        sector_etfs.insert("Healthcare".to_string(), "XLV".to_string());
        sector_etfs.insert("Consumer Discretionary".to_string(), "XLY".to_string());
        sector_etfs.insert("Consumer Staples".to_string(), "XLP".to_string());
        sector_etfs.insert("Energy".to_string(), "XLE".to_string());
        sector_etfs.insert("Industrial".to_string(), "XLI".to_string());
        sector_etfs.insert("Materials".to_string(), "XLB".to_string());
        sector_etfs.insert("Real Estate".to_string(), "XLRE".to_string());
        sector_etfs.insert("Utilities".to_string(), "XLU".to_string());
        sector_etfs.insert("Communication".to_string(), "XLC".to_string());

        let mut stock_sectors = std::collections::HashMap::new();
        // Tech
        stock_sectors.insert("AAPL".to_string(), "Technology".to_string());
        stock_sectors.insert("MSFT".to_string(), "Technology".to_string());
        stock_sectors.insert("GOOGL".to_string(), "Communication".to_string());
        stock_sectors.insert("GOOG".to_string(), "Communication".to_string());
        stock_sectors.insert("META".to_string(), "Communication".to_string());
        stock_sectors.insert("NVDA".to_string(), "Technology".to_string());
        stock_sectors.insert("TSLA".to_string(), "Consumer Discretionary".to_string());
        stock_sectors.insert("AMZN".to_string(), "Consumer Discretionary".to_string());
        // Finance
        stock_sectors.insert("JPM".to_string(), "Financial".to_string());
        stock_sectors.insert("BAC".to_string(), "Financial".to_string());
        stock_sectors.insert("GS".to_string(), "Financial".to_string());
        stock_sectors.insert("MS".to_string(), "Financial".to_string());
        // Healthcare
        stock_sectors.insert("JNJ".to_string(), "Healthcare".to_string());
        stock_sectors.insert("PFE".to_string(), "Healthcare".to_string());
        stock_sectors.insert("UNH".to_string(), "Healthcare".to_string());
        stock_sectors.insert("MRK".to_string(), "Healthcare".to_string());
        // Energy
        stock_sectors.insert("XOM".to_string(), "Energy".to_string());
        stock_sectors.insert("CVX".to_string(), "Energy".to_string());

        let mut substitutes = std::collections::HashMap::new();

        // AAPL substitutes
        substitutes.insert(
            "AAPL".to_string(),
            vec![
                SubstituteSecurity {
                    symbol: "MSFT".to_string(),
                    name: "Microsoft Corporation".to_string(),
                    substitute_type: SubstituteType::Competitor,
                    correlation: CorrelationScore::calculate(0.82, 0.85, 0.9),
                    expense_ratio: None,
                    wash_sale_safe: true,
                    reason: "Large-cap tech with similar volatility profile".to_string(),
                    risk_comparison: "Similar risk profile".to_string(),
                },
                SubstituteSecurity {
                    symbol: "QQQ".to_string(),
                    name: "Invesco QQQ Trust".to_string(),
                    substitute_type: SubstituteType::IndexFund,
                    correlation: CorrelationScore::calculate(0.88, 0.90, 0.8),
                    expense_ratio: Some(0.20),
                    wash_sale_safe: true,
                    reason: "NASDAQ-100 index with AAPL as top holding".to_string(),
                    risk_comparison: "Diversified, lower single-stock risk".to_string(),
                },
            ],
        );

        // MSFT substitutes
        substitutes.insert(
            "MSFT".to_string(),
            vec![
                SubstituteSecurity {
                    symbol: "AAPL".to_string(),
                    name: "Apple Inc.".to_string(),
                    substitute_type: SubstituteType::Competitor,
                    correlation: CorrelationScore::calculate(0.82, 0.85, 0.9),
                    expense_ratio: None,
                    wash_sale_safe: true,
                    reason: "Large-cap tech with similar volatility profile".to_string(),
                    risk_comparison: "Similar risk profile".to_string(),
                },
                SubstituteSecurity {
                    symbol: "VGT".to_string(),
                    name: "Vanguard Information Technology ETF".to_string(),
                    substitute_type: SubstituteType::SectorETF,
                    correlation: CorrelationScore::calculate(0.90, 0.88, 1.0),
                    expense_ratio: Some(0.10),
                    wash_sale_safe: true,
                    reason: "Broad tech sector with MSFT exposure".to_string(),
                    risk_comparison: "Diversified tech exposure".to_string(),
                },
            ],
        );

        // S&P 500 ETF substitutes
        substitutes.insert(
            "SPY".to_string(),
            vec![
                SubstituteSecurity {
                    symbol: "VOO".to_string(),
                    name: "Vanguard S&P 500 ETF".to_string(),
                    substitute_type: SubstituteType::IndexFund,
                    correlation: CorrelationScore::calculate(0.99, 0.99, 1.0),
                    expense_ratio: Some(0.03),
                    wash_sale_safe: false, // Substantially identical!
                    reason: "Tracks same index, lower expense ratio".to_string(),
                    risk_comparison: "Identical exposure".to_string(),
                },
                SubstituteSecurity {
                    symbol: "IVV".to_string(),
                    name: "iShares Core S&P 500 ETF".to_string(),
                    substitute_type: SubstituteType::IndexFund,
                    correlation: CorrelationScore::calculate(0.99, 0.99, 1.0),
                    expense_ratio: Some(0.03),
                    wash_sale_safe: false, // Substantially identical!
                    reason: "Tracks same index".to_string(),
                    risk_comparison: "Identical exposure".to_string(),
                },
                SubstituteSecurity {
                    symbol: "VTI".to_string(),
                    name: "Vanguard Total Stock Market ETF".to_string(),
                    substitute_type: SubstituteType::IndexFund,
                    correlation: CorrelationScore::calculate(0.97, 0.98, 0.85),
                    expense_ratio: Some(0.03),
                    wash_sale_safe: true,
                    reason: "Total US market, broader than S&P 500".to_string(),
                    risk_comparison: "Slightly more small-cap exposure".to_string(),
                },
            ],
        );

        let identical_pairs = vec![
            ("SPY".to_string(), "VOO".to_string()),
            ("SPY".to_string(), "IVV".to_string()),
            ("VOO".to_string(), "IVV".to_string()),
            ("GOOGL".to_string(), "GOOG".to_string()),
            ("BRK.A".to_string(), "BRK.B".to_string()),
        ];

        Self {
            sector_etfs,
            stock_sectors,
            substitutes,
            identical_pairs,
        }
    }
}

impl SubstitutesDatabase {
    fn get_substitutes(&self, symbol: &str) -> Option<Vec<SubstituteSecurity>> {
        self.substitutes.get(symbol).cloned()
    }

    fn get_sector(&self, symbol: &str) -> Option<&String> {
        self.stock_sectors.get(symbol)
    }

    fn get_sector_etf(&self, sector: &str) -> Option<&String> {
        self.sector_etfs.get(sector)
    }

    fn are_identical(&self, symbol1: &str, symbol2: &str) -> bool {
        for (a, b) in &self.identical_pairs {
            if (a == symbol1 && b == symbol2) || (a == symbol2 && b == symbol1) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_substitutes() {
        let finder = SubstituteFinder::new();
        let subs = finder.find_substitutes("AAPL");

        assert!(!subs.is_empty());
        // Should find MSFT or QQQ
        assert!(subs.iter().any(|s| s.symbol == "MSFT" || s.symbol == "QQQ"));
    }

    #[test]
    fn test_substantially_identical() {
        let finder = SubstituteFinder::new();

        // Same symbol
        assert!(finder.is_substantially_identical("SPY", "SPY"));

        // Known identical pairs
        assert!(finder.is_substantially_identical("SPY", "VOO"));
        assert!(finder.is_substantially_identical("GOOGL", "GOOG"));

        // Different securities
        assert!(!finder.is_substantially_identical("AAPL", "MSFT"));
    }

    #[test]
    fn test_wash_sale_safe() {
        let finder = SubstituteFinder::new();
        let subs = finder.find_substitutes("SPY");

        // VTI should be wash-sale safe
        let vti = subs.iter().find(|s| s.symbol == "VTI");
        assert!(vti.is_some_and(|s| s.wash_sale_safe));

        // VOO should NOT be wash-sale safe (substantially identical)
        let voo = subs.iter().find(|s| s.symbol == "VOO");
        assert!(voo.is_none_or(|s| !s.wash_sale_safe));
    }
}
