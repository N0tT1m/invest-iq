//! Session Scoring
//!
//! Scores user decisions against AI recommendations and optimal play.

use crate::replay::{TimeMachineSession, TradeAction, UserDecision};
use serde::{Deserialize, Serialize};

/// Score for an individual decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionScore {
    /// Date of decision
    pub decision_date: chrono::NaiveDate,
    /// User's action
    pub user_action: TradeAction,
    /// AI's recommendation
    pub ai_action: TradeAction,
    /// Actual return following decision
    pub actual_return_pct: f64,
    /// Points earned for this decision
    pub points: i32,
    /// Whether user matched AI recommendation
    pub matched_ai: bool,
    /// Whether decision was profitable
    pub was_profitable: bool,
    /// Feedback message
    pub feedback: String,
}

/// Complete scorecard for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreCard {
    /// Session ID
    pub session_id: String,
    /// Scenario name (if applicable)
    pub scenario_name: Option<String>,
    /// Total points earned
    pub total_points: i32,
    /// Maximum possible points
    pub max_points: i32,
    /// Percentage score
    pub score_pct: f64,
    /// User's total return
    pub user_return_pct: f64,
    /// Buy-and-hold return for comparison
    pub buy_hold_return_pct: f64,
    /// AI strategy return (if followed perfectly)
    pub ai_return_pct: f64,
    /// Number of correct decisions
    pub correct_decisions: u32,
    /// Total decisions made
    pub total_decisions: u32,
    /// Decision accuracy percentage
    pub accuracy_pct: f64,
    /// Individual decision scores
    pub decision_scores: Vec<DecisionScore>,
    /// Grade (A, B, C, D, F)
    pub grade: String,
    /// Summary feedback
    pub summary: String,
    /// Strengths identified
    pub strengths: Vec<String>,
    /// Areas for improvement
    pub improvements: Vec<String>,
    /// Rank compared to other attempts (if available)
    pub rank: Option<u32>,
    /// Percentile (if available)
    pub percentile: Option<f64>,
}

/// Session scorer
pub struct SessionScorer {
    /// Points for matching profitable AI decision
    pub points_ai_match_profit: i32,
    /// Points for profitable decision that differed from AI
    pub points_user_profit: i32,
    /// Points for holding when hold was best
    pub points_correct_hold: i32,
    /// Penalty for unprofitable decision
    pub penalty_loss: i32,
}

impl Default for SessionScorer {
    fn default() -> Self {
        Self {
            points_ai_match_profit: 100,
            points_user_profit: 75,
            points_correct_hold: 50,
            penalty_loss: -25,
        }
    }
}

impl SessionScorer {
    /// Create a new scorer with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Score a completed session
    pub fn score_session(
        &self,
        session: &TimeMachineSession,
        first_price: f64,
        last_price: f64,
    ) -> ScoreCard {
        let mut decision_scores = Vec::new();
        let mut total_points = 0i32;
        let mut correct_decisions = 0u32;

        // Score each decision
        for decision in &session.decisions {
            let score = self.score_decision(decision);
            if score.was_profitable || score.matched_ai {
                correct_decisions += 1;
            }
            total_points += score.points;
            decision_scores.push(score);
        }

        let total_decisions = session.decisions.len() as u32;
        let max_points = total_decisions as i32 * self.points_ai_match_profit;

        // Calculate returns
        let user_return_pct = session.total_return_pct();
        let buy_hold_return_pct = ((last_price - first_price) / first_price) * 100.0;

        // Estimate AI return (would need to simulate AI strategy)
        let ai_return_pct = self.estimate_ai_return(&session.decisions);

        // Calculate accuracy
        let accuracy_pct = if total_decisions > 0 {
            (correct_decisions as f64 / total_decisions as f64) * 100.0
        } else {
            0.0
        };

        // Calculate score percentage
        let score_pct = if max_points > 0 {
            ((total_points.max(0) as f64) / max_points as f64) * 100.0
        } else {
            0.0
        };

        // Determine grade
        let grade = self.calculate_grade(score_pct, user_return_pct, buy_hold_return_pct);

        // Generate feedback
        let (summary, strengths, improvements) =
            self.generate_feedback(&decision_scores, user_return_pct, buy_hold_return_pct);

        ScoreCard {
            session_id: session.id.clone(),
            scenario_name: session.scenario_id.clone(),
            total_points,
            max_points,
            score_pct,
            user_return_pct,
            buy_hold_return_pct,
            ai_return_pct,
            correct_decisions,
            total_decisions,
            accuracy_pct,
            decision_scores,
            grade,
            summary,
            strengths,
            improvements,
            rank: None,
            percentile: None,
        }
    }

    /// Score an individual decision
    fn score_decision(&self, decision: &UserDecision) -> DecisionScore {
        let actual_return = decision.actual_return.unwrap_or(0.0);
        let matched_ai = decision.action == decision.ai_recommendation;

        // Determine if decision was profitable
        let was_profitable = match decision.action {
            TradeAction::Buy => actual_return > 0.0,
            TradeAction::Sell => actual_return < 0.0,
            TradeAction::Hold => actual_return.abs() < 1.0, // Hold was right if small move
        };

        // Calculate points
        let points = if matched_ai && was_profitable {
            self.points_ai_match_profit
        } else if was_profitable {
            self.points_user_profit
        } else if decision.action == TradeAction::Hold && actual_return.abs() < 0.5 {
            self.points_correct_hold
        } else {
            self.penalty_loss
        };

        // Generate feedback
        let feedback = self.generate_decision_feedback(decision, was_profitable, matched_ai);

        DecisionScore {
            decision_date: decision.decision_date,
            user_action: decision.action,
            ai_action: decision.ai_recommendation,
            actual_return_pct: actual_return,
            points,
            matched_ai,
            was_profitable,
            feedback,
        }
    }

    /// Generate feedback for a decision
    fn generate_decision_feedback(
        &self,
        decision: &UserDecision,
        was_profitable: bool,
        matched_ai: bool,
    ) -> String {
        let actual = decision.actual_return.unwrap_or(0.0);

        if matched_ai && was_profitable {
            format!(
                "Excellent! Your {:?} matched AI and captured {:.1}% return.",
                decision.action, actual.abs()
            )
        } else if was_profitable && !matched_ai {
            format!(
                "Good call! Your {:?} beat AI's {:?} recommendation with {:.1}% return.",
                decision.action, decision.ai_recommendation, actual.abs()
            )
        } else if matched_ai && !was_profitable {
            format!(
                "You matched AI, but the market moved against us ({:.1}%). Even good analysis can't predict everything.",
                actual
            )
        } else {
            format!(
                "AI recommended {:?} which would have been better. Market moved {:.1}%.",
                decision.ai_recommendation, actual
            )
        }
    }

    /// Calculate letter grade
    fn calculate_grade(&self, score_pct: f64, user_return: f64, buy_hold_return: f64) -> String {
        // Base grade on score percentage
        let base_grade = if score_pct >= 90.0 {
            "A"
        } else if score_pct >= 80.0 {
            "B"
        } else if score_pct >= 70.0 {
            "C"
        } else if score_pct >= 60.0 {
            "D"
        } else {
            "F"
        };

        // Adjust based on beating buy-and-hold
        let outperformed = user_return > buy_hold_return;

        match (base_grade, outperformed) {
            ("A", true) => "A+".to_string(),
            ("B", true) => "A-".to_string(),
            ("C", true) => "B-".to_string(),
            ("D", true) => "C".to_string(),
            ("F", true) => "D".to_string(),
            (grade, _) => grade.to_string(),
        }
    }

    /// Estimate AI return if user had followed all recommendations
    fn estimate_ai_return(&self, decisions: &[UserDecision]) -> f64 {
        // Simplified estimation
        let mut ai_return = 0.0;
        for decision in decisions {
            let actual = decision.actual_return.unwrap_or(0.0);
            match decision.ai_recommendation {
                TradeAction::Buy => ai_return += actual,
                TradeAction::Sell => ai_return -= actual,
                TradeAction::Hold => {} // No change
            }
        }
        ai_return
    }

    /// Generate summary feedback
    fn generate_feedback(
        &self,
        scores: &[DecisionScore],
        user_return: f64,
        buy_hold_return: f64,
    ) -> (String, Vec<String>, Vec<String>) {
        let mut strengths = Vec::new();
        let mut improvements = Vec::new();

        // Analyze decision patterns
        let profitable_count = scores.iter().filter(|s| s.was_profitable).count();
        let matched_count = scores.iter().filter(|s| s.matched_ai).count();
        let total = scores.len();

        if total > 0 {
            let profit_rate = (profitable_count as f64 / total as f64) * 100.0;
            let match_rate = (matched_count as f64 / total as f64) * 100.0;

            if profit_rate > 60.0 {
                strengths.push("Strong profitability rate".to_string());
            }
            if match_rate > 70.0 {
                strengths.push("Good alignment with AI signals".to_string());
            }
            if profit_rate < 40.0 {
                improvements.push("Focus on cutting losses earlier".to_string());
            }
            if match_rate < 50.0 {
                improvements.push("Consider following AI recommendations more closely".to_string());
            }
        }

        // Analyze return vs buy-and-hold
        if user_return > buy_hold_return {
            strengths.push(format!(
                "Beat buy-and-hold by {:.1}%",
                user_return - buy_hold_return
            ));
        } else {
            improvements.push(format!(
                "Underperformed buy-and-hold by {:.1}%",
                buy_hold_return - user_return
            ));
        }

        // Look for specific patterns
        let consecutive_losses = self.count_consecutive_losses(scores);
        if consecutive_losses >= 3 {
            improvements.push(format!(
                "Had {} consecutive losing decisions - consider stepping back after losses",
                consecutive_losses
            ));
        }

        // Generate summary
        let summary = if user_return > buy_hold_return && profitable_count > total / 2 {
            "Strong performance! You beat the market with mostly profitable decisions.".to_string()
        } else if user_return > buy_hold_return {
            "You beat the market, though with mixed decision quality. Focus on consistency."
                .to_string()
        } else if profitable_count > total / 2 {
            "Mostly profitable decisions, but timing could be improved to beat buy-and-hold."
                .to_string()
        } else {
            "Challenging scenario! Review the AI recommendations to understand what signals to watch for.".to_string()
        };

        (summary, strengths, improvements)
    }

    /// Count maximum consecutive losses
    fn count_consecutive_losses(&self, scores: &[DecisionScore]) -> usize {
        let mut max_streak = 0;
        let mut current_streak = 0;

        for score in scores {
            if !score.was_profitable {
                current_streak += 1;
                max_streak = max_streak.max(current_streak);
            } else {
                current_streak = 0;
            }
        }

        max_streak
    }
}

/// Leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    /// User ID (may be anonymous)
    pub user_id: String,
    /// Display name
    pub display_name: String,
    /// Scenario ID
    pub scenario_id: String,
    /// Score achieved
    pub score: i32,
    /// Return achieved
    pub return_pct: f64,
    /// Grade
    pub grade: String,
    /// Date completed
    pub completed_at: chrono::DateTime<chrono::Utc>,
    /// Rank
    pub rank: u32,
}

/// Leaderboard for a scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leaderboard {
    /// Scenario ID
    pub scenario_id: String,
    /// Scenario name
    pub scenario_name: String,
    /// Top entries
    pub entries: Vec<LeaderboardEntry>,
    /// Total participants
    pub total_participants: u32,
    /// Average score
    pub average_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_scorer_default() {
        let scorer = SessionScorer::new();
        assert_eq!(scorer.points_ai_match_profit, 100);
    }

    #[test]
    fn test_decision_scoring() {
        let scorer = SessionScorer::new();
        let decision = UserDecision {
            decision_date: NaiveDate::from_ymd_opt(2020, 3, 10).unwrap(),
            action: TradeAction::Buy,
            shares: Some(10),
            price: 100.0,
            ai_recommendation: TradeAction::Buy,
            actual_return: Some(5.0),
            reason: None,
        };

        let score = scorer.score_decision(&decision);
        assert!(score.matched_ai);
        assert!(score.was_profitable);
        assert_eq!(score.points, 100);
    }

    #[test]
    fn test_grade_calculation() {
        let scorer = SessionScorer::new();

        assert_eq!(scorer.calculate_grade(95.0, 10.0, 5.0), "A+");
        assert_eq!(scorer.calculate_grade(85.0, 10.0, 5.0), "A-");
        assert_eq!(scorer.calculate_grade(75.0, 5.0, 10.0), "C");
        assert_eq!(scorer.calculate_grade(55.0, 5.0, 10.0), "F");
    }
}
