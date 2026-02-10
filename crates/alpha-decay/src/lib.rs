//! Alpha Decay Monitor
//!
//! Tracks when trading strategies start degrading over time.
//! Implements change detection algorithms and health reporting.

pub mod monitor;
pub mod change_detector;
pub mod health_report;

pub use monitor::{AlphaDecayMonitor, StrategyPerformance, PerformanceSnapshot, DecayMetrics};
pub use change_detector::{ChangeDetector, ChangePoint, CusumResult};
pub use health_report::{HealthReport, HealthReportBuilder, HealthStatus, StrategyHealth, DecayAlert};
