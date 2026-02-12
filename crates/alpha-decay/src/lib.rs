//! Alpha Decay Monitor
//!
//! Tracks when trading strategies start degrading over time.
//! Implements change detection algorithms and health reporting.

pub mod change_detector;
pub mod health_report;
pub mod monitor;

pub use change_detector::{ChangeDetector, ChangePoint, CusumResult};
pub use health_report::{
    DecayAlert, HealthReport, HealthReportBuilder, HealthStatus, StrategyHealth,
};
pub use monitor::{AlphaDecayMonitor, DecayMetrics, PerformanceSnapshot, StrategyPerformance};
