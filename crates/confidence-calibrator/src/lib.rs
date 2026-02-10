//! Confidence Calibrator Module
//!
//! Provides uncertainty-aware predictions with calibration to show how reliable signals are.
//! Implements Platt scaling and isotonic regression for probability calibration.

pub mod calibrator;
pub mod history;
pub mod uncertainty;

pub use calibrator::{
    CalibrationMethod, CalibratedPrediction, ConfidenceCalibrator, CalibrationStats,
};
pub use history::{CalibrationHistory, CalibrationHistoryStore, PredictionOutcome};
pub use uncertainty::{
    UncertaintyAnalysis, UncertaintyDecomposition, UncertaintyEstimator, UncertaintyLevel,
    PredictionContext,
};
