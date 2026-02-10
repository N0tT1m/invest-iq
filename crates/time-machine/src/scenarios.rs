//! Pre-built Historical Scenarios
//!
//! Famous market events for learning and practice.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Scenario difficulty level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    /// Easy - clear trends, predictable patterns
    Beginner,
    /// Medium - some volatility, mixed signals
    Intermediate,
    /// Hard - high volatility, unexpected moves
    Advanced,
    /// Expert - extreme conditions, unpredictable
    Expert,
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difficulty::Beginner => write!(f, "Beginner"),
            Difficulty::Intermediate => write!(f, "Intermediate"),
            Difficulty::Advanced => write!(f, "Advanced"),
            Difficulty::Expert => write!(f, "Expert"),
        }
    }
}

/// A historical scenario for replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Detailed description
    pub description: String,
    /// What users will learn
    pub learning_objectives: Vec<String>,
    /// Start date
    pub start_date: NaiveDate,
    /// End date
    pub end_date: NaiveDate,
    /// Primary symbol
    pub primary_symbol: String,
    /// Additional symbols for context
    pub symbols: Vec<String>,
    /// Difficulty level
    pub difficulty: Difficulty,
    /// Category/type
    pub category: ScenarioCategory,
    /// Historical context (shown before starting)
    pub context: String,
    /// Key events during the period
    pub key_events: Vec<KeyEvent>,
    /// Estimated time to complete (minutes)
    pub estimated_duration_minutes: u32,
    /// Maximum score possible
    pub max_score: u32,
    /// Whether this scenario is featured
    pub is_featured: bool,
    /// Number of times completed by users
    pub completion_count: u32,
    /// Average score achieved
    pub average_score: Option<f64>,
}

/// Category of scenario
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioCategory {
    /// Market crashes
    Crash,
    /// Recovery rallies
    Recovery,
    /// High volatility events
    Volatility,
    /// Sector rotation
    Rotation,
    /// Meme stock phenomena
    MemeStock,
    /// Interest rate events
    RateEvent,
    /// Earnings surprises
    Earnings,
    /// Geopolitical events
    Geopolitical,
}

impl std::fmt::Display for ScenarioCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScenarioCategory::Crash => write!(f, "Crash"),
            ScenarioCategory::Recovery => write!(f, "Recovery"),
            ScenarioCategory::Volatility => write!(f, "High Volatility"),
            ScenarioCategory::Rotation => write!(f, "Sector Rotation"),
            ScenarioCategory::MemeStock => write!(f, "Meme Stock"),
            ScenarioCategory::RateEvent => write!(f, "Rate Event"),
            ScenarioCategory::Earnings => write!(f, "Earnings"),
            ScenarioCategory::Geopolitical => write!(f, "Geopolitical"),
        }
    }
}

/// A key event during the scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEvent {
    /// Date of the event
    pub date: NaiveDate,
    /// Short title
    pub title: String,
    /// Description
    pub description: String,
    /// Impact on market
    pub impact: EventImpact,
}

/// Impact level of an event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventImpact {
    Low,
    Medium,
    High,
    Extreme,
}

/// Library of pre-built scenarios
pub struct ScenarioLibrary;

impl ScenarioLibrary {
    /// Get all available scenarios
    pub fn all_scenarios() -> Vec<Scenario> {
        vec![
            Self::covid_crash(),
            Self::covid_recovery(),
            Self::gamestop_squeeze(),
            Self::financial_crisis_2008(),
            Self::tech_selloff_2022(),
            Self::meme_stock_era(),
        ]
    }

    /// Get featured scenarios
    pub fn featured_scenarios() -> Vec<Scenario> {
        Self::all_scenarios()
            .into_iter()
            .filter(|s| s.is_featured)
            .collect()
    }

    /// Get scenario by ID
    pub fn get_scenario(id: &str) -> Option<Scenario> {
        Self::all_scenarios().into_iter().find(|s| s.id == id)
    }

    /// Get scenarios by difficulty
    pub fn by_difficulty(difficulty: Difficulty) -> Vec<Scenario> {
        Self::all_scenarios()
            .into_iter()
            .filter(|s| s.difficulty == difficulty)
            .collect()
    }

    /// Get scenarios by category
    pub fn by_category(category: ScenarioCategory) -> Vec<Scenario> {
        Self::all_scenarios()
            .into_iter()
            .filter(|s| s.category == category)
            .collect()
    }

    /// COVID-19 Market Crash (March 2020)
    pub fn covid_crash() -> Scenario {
        Scenario {
            id: "covid-crash-2020".to_string(),
            name: "COVID-19 Crash".to_string(),
            description: "The fastest bear market in history. Markets dropped 34% in just 23 trading days as the COVID-19 pandemic triggered global lockdowns.".to_string(),
            learning_objectives: vec![
                "Recognize panic selling patterns".to_string(),
                "Understand circuit breaker mechanisms".to_string(),
                "Learn to identify capitulation signals".to_string(),
                "Practice risk management during crashes".to_string(),
            ],
            start_date: NaiveDate::from_ymd_opt(2020, 2, 19).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2020, 3, 23).unwrap(),
            primary_symbol: "SPY".to_string(),
            symbols: vec!["SPY".to_string(), "QQQ".to_string(), "VIX".to_string()],
            difficulty: Difficulty::Advanced,
            category: ScenarioCategory::Crash,
            context: "In February 2020, the S&P 500 hit all-time highs. COVID-19 was spreading in China but markets largely ignored it. Then Italy locked down, and the dominoes began to fall.".to_string(),
            key_events: vec![
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2020, 2, 24).unwrap(),
                    title: "Italy Lockdown".to_string(),
                    description: "Italy announces lockdown of northern regions".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2020, 3, 9).unwrap(),
                    title: "Circuit Breaker #1".to_string(),
                    description: "First trading halt since 1997 as S&P drops 7%".to_string(),
                    impact: EventImpact::Extreme,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2020, 3, 12).unwrap(),
                    title: "Circuit Breaker #2".to_string(),
                    description: "Second circuit breaker, worst day since 1987".to_string(),
                    impact: EventImpact::Extreme,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2020, 3, 16).unwrap(),
                    title: "Circuit Breaker #3".to_string(),
                    description: "Third circuit breaker, Fed cuts rates to zero".to_string(),
                    impact: EventImpact::Extreme,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2020, 3, 23).unwrap(),
                    title: "Market Bottom".to_string(),
                    description: "S&P 500 bottoms at 2,237, Fed announces unlimited QE".to_string(),
                    impact: EventImpact::High,
                },
            ],
            estimated_duration_minutes: 30,
            max_score: 1000,
            is_featured: true,
            completion_count: 0,
            average_score: None,
        }
    }

    /// COVID Recovery Rally (March-June 2020)
    pub fn covid_recovery() -> Scenario {
        Scenario {
            id: "covid-recovery-2020".to_string(),
            name: "V-Shaped Recovery".to_string(),
            description: "One of the fastest recoveries in market history. The S&P 500 gained 45% from March lows in under 3 months.".to_string(),
            learning_objectives: vec![
                "Recognize recovery patterns".to_string(),
                "Understand the role of monetary policy".to_string(),
                "Learn to identify sector rotation".to_string(),
                "Practice buying into fear".to_string(),
            ],
            start_date: NaiveDate::from_ymd_opt(2020, 3, 23).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2020, 6, 8).unwrap(),
            primary_symbol: "SPY".to_string(),
            symbols: vec!["SPY".to_string(), "QQQ".to_string(), "TSLA".to_string(), "AMZN".to_string()],
            difficulty: Difficulty::Intermediate,
            category: ScenarioCategory::Recovery,
            context: "After the March 23rd bottom, unprecedented fiscal and monetary stimulus flooded the markets. Work-from-home stocks soared while travel and hospitality languished.".to_string(),
            key_events: vec![
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2020, 3, 27).unwrap(),
                    title: "CARES Act".to_string(),
                    description: "$2.2 trillion stimulus package signed".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2020, 4, 9).unwrap(),
                    title: "Fed Expands".to_string(),
                    description: "Fed announces $2.3T in additional lending".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2020, 5, 18).unwrap(),
                    title: "Moderna Vaccine".to_string(),
                    description: "Moderna announces positive vaccine trial results".to_string(),
                    impact: EventImpact::Medium,
                },
            ],
            estimated_duration_minutes: 25,
            max_score: 800,
            is_featured: true,
            completion_count: 0,
            average_score: None,
        }
    }

    /// GameStop Short Squeeze (January 2021)
    pub fn gamestop_squeeze() -> Scenario {
        Scenario {
            id: "gme-squeeze-2021".to_string(),
            name: "GameStop Squeeze".to_string(),
            description: "Retail traders vs. Wall Street. GME went from $20 to $483 in two weeks as short sellers were forced to cover.".to_string(),
            learning_objectives: vec![
                "Understand short squeeze mechanics".to_string(),
                "Recognize FOMO and parabolic moves".to_string(),
                "Learn about options gamma squeezes".to_string(),
                "Practice discipline during mania".to_string(),
            ],
            start_date: NaiveDate::from_ymd_opt(2021, 1, 11).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2021, 2, 5).unwrap(),
            primary_symbol: "GME".to_string(),
            symbols: vec!["GME".to_string(), "AMC".to_string(), "BB".to_string(), "NOK".to_string()],
            difficulty: Difficulty::Expert,
            category: ScenarioCategory::MemeStock,
            context: "GameStop was a failing retailer with 140% short interest. A movement on Reddit's WallStreetBets targeted the stock, triggering a historic short squeeze.".to_string(),
            key_events: vec![
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2021, 1, 13).unwrap(),
                    title: "Ryan Cohen Tweet".to_string(),
                    description: "Ryan Cohen tweets, stock doubles".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2021, 1, 22).unwrap(),
                    title: "Gamma Squeeze".to_string(),
                    description: "Options gamma squeeze begins, stock hits $76".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2021, 1, 28).unwrap(),
                    title: "Trading Halted".to_string(),
                    description: "Robinhood restricts buying, controversy erupts".to_string(),
                    impact: EventImpact::Extreme,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2021, 2, 2).unwrap(),
                    title: "Silver Distraction".to_string(),
                    description: "Media claims squeeze moves to silver, GME crashes".to_string(),
                    impact: EventImpact::Medium,
                },
            ],
            estimated_duration_minutes: 20,
            max_score: 1200,
            is_featured: true,
            completion_count: 0,
            average_score: None,
        }
    }

    /// 2008 Financial Crisis
    pub fn financial_crisis_2008() -> Scenario {
        Scenario {
            id: "financial-crisis-2008".to_string(),
            name: "2008 Financial Crisis".to_string(),
            description: "The worst financial crisis since the Great Depression. Lehman Brothers collapsed, banks failed, and the global economy teetered on the brink.".to_string(),
            learning_objectives: vec![
                "Understand systemic financial risk".to_string(),
                "Recognize credit crisis symptoms".to_string(),
                "Learn about contagion effects".to_string(),
                "Practice patience during prolonged downturns".to_string(),
            ],
            start_date: NaiveDate::from_ymd_opt(2008, 9, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2008, 11, 20).unwrap(),
            primary_symbol: "SPY".to_string(),
            symbols: vec!["SPY".to_string(), "XLF".to_string(), "BAC".to_string(), "GS".to_string()],
            difficulty: Difficulty::Advanced,
            category: ScenarioCategory::Crash,
            context: "The subprime mortgage crisis had been building for a year. In September 2008, the situation went from bad to catastrophic.".to_string(),
            key_events: vec![
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2008, 9, 7).unwrap(),
                    title: "Fannie/Freddie Seized".to_string(),
                    description: "Government takes over Fannie Mae and Freddie Mac".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2008, 9, 15).unwrap(),
                    title: "Lehman Bankruptcy".to_string(),
                    description: "Lehman Brothers files for bankruptcy".to_string(),
                    impact: EventImpact::Extreme,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2008, 9, 16).unwrap(),
                    title: "AIG Bailout".to_string(),
                    description: "Fed bails out AIG with $85 billion".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2008, 10, 3).unwrap(),
                    title: "TARP Passed".to_string(),
                    description: "$700 billion bailout signed into law".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2008, 10, 13).unwrap(),
                    title: "Bank Recapitalization".to_string(),
                    description: "Treasury announces plan to inject capital into banks".to_string(),
                    impact: EventImpact::High,
                },
            ],
            estimated_duration_minutes: 35,
            max_score: 1000,
            is_featured: true,
            completion_count: 0,
            average_score: None,
        }
    }

    /// 2022 Tech Selloff
    pub fn tech_selloff_2022() -> Scenario {
        Scenario {
            id: "tech-selloff-2022".to_string(),
            name: "2022 Tech Selloff".to_string(),
            description: "Rising rates crushed high-growth tech stocks. The Nasdaq dropped 33% as the Fed aggressively fought inflation.".to_string(),
            learning_objectives: vec![
                "Understand interest rate sensitivity".to_string(),
                "Recognize growth vs. value rotation".to_string(),
                "Learn about multiple compression".to_string(),
                "Practice cutting losses early".to_string(),
            ],
            start_date: NaiveDate::from_ymd_opt(2022, 1, 3).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2022, 6, 16).unwrap(),
            primary_symbol: "QQQ".to_string(),
            symbols: vec!["QQQ".to_string(), "META".to_string(), "NFLX".to_string(), "ARKK".to_string()],
            difficulty: Difficulty::Intermediate,
            category: ScenarioCategory::Crash,
            context: "After years of near-zero rates, the Fed began hiking aggressively to combat inflation. High-growth tech stocks, which had soared during COVID, were hit hardest.".to_string(),
            key_events: vec![
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2022, 1, 26).unwrap(),
                    title: "Fed Hawkish Pivot".to_string(),
                    description: "Fed signals faster rate hikes, tech tumbles".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2022, 2, 3).unwrap(),
                    title: "Meta Crashes".to_string(),
                    description: "META drops 26% on earnings, loses $230B in value".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2022, 4, 20).unwrap(),
                    title: "Netflix Plunges".to_string(),
                    description: "NFLX drops 35% on subscriber losses".to_string(),
                    impact: EventImpact::Medium,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2022, 5, 4).unwrap(),
                    title: "50bp Hike".to_string(),
                    description: "Fed raises rates 50bp, largest since 2000".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2022, 6, 15).unwrap(),
                    title: "75bp Hike".to_string(),
                    description: "Fed raises 75bp, largest since 1994".to_string(),
                    impact: EventImpact::High,
                },
            ],
            estimated_duration_minutes: 40,
            max_score: 900,
            is_featured: false,
            completion_count: 0,
            average_score: None,
        }
    }

    /// Meme Stock Era (January-March 2021)
    pub fn meme_stock_era() -> Scenario {
        Scenario {
            id: "meme-stock-era-2021".to_string(),
            name: "Meme Stock Era".to_string(),
            description: "Beyond GameStop - the entire meme stock phenomenon. AMC, BBBY, and others experienced massive retail-driven moves.".to_string(),
            learning_objectives: vec![
                "Identify meme stock characteristics".to_string(),
                "Understand social media influence".to_string(),
                "Recognize pump and dump patterns".to_string(),
                "Practice independent thinking".to_string(),
            ],
            start_date: NaiveDate::from_ymd_opt(2021, 1, 25).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2021, 3, 15).unwrap(),
            primary_symbol: "AMC".to_string(),
            symbols: vec!["AMC".to_string(), "GME".to_string(), "BBBY".to_string(), "KOSS".to_string()],
            difficulty: Difficulty::Intermediate,
            category: ScenarioCategory::MemeStock,
            context: "The GameStop squeeze opened the floodgates. Retail traders piled into other heavily shorted stocks, creating a series of spectacular but volatile moves.".to_string(),
            key_events: vec![
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2021, 1, 27).unwrap(),
                    title: "AMC Surges".to_string(),
                    description: "AMC gains 300% in one day".to_string(),
                    impact: EventImpact::High,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2021, 2, 1).unwrap(),
                    title: "Silver Squeeze".to_string(),
                    description: "Attempted silver squeeze fails".to_string(),
                    impact: EventImpact::Low,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2021, 2, 24).unwrap(),
                    title: "GME Round 2".to_string(),
                    description: "GME surges from $40 to $91 after Roaring Kitty testimony".to_string(),
                    impact: EventImpact::Medium,
                },
                KeyEvent {
                    date: NaiveDate::from_ymd_opt(2021, 3, 10).unwrap(),
                    title: "Flash Crash".to_string(),
                    description: "GME drops 40% in 25 minutes, recovers".to_string(),
                    impact: EventImpact::High,
                },
            ],
            estimated_duration_minutes: 25,
            max_score: 850,
            is_featured: false,
            completion_count: 0,
            average_score: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_scenarios_exist() {
        let scenarios = ScenarioLibrary::all_scenarios();
        assert!(!scenarios.is_empty());
        assert!(scenarios.len() >= 6);
    }

    #[test]
    fn test_featured_scenarios() {
        let featured = ScenarioLibrary::featured_scenarios();
        assert!(featured.len() >= 3);
        assert!(featured.iter().all(|s| s.is_featured));
    }

    #[test]
    fn test_get_scenario_by_id() {
        let scenario = ScenarioLibrary::get_scenario("covid-crash-2020");
        assert!(scenario.is_some());
        assert_eq!(scenario.unwrap().name, "COVID-19 Crash");
    }

    #[test]
    fn test_scenario_dates_valid() {
        for scenario in ScenarioLibrary::all_scenarios() {
            assert!(scenario.start_date < scenario.end_date);
        }
    }
}
