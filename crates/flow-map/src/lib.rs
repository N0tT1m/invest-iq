//! Flow Map Module
//!
//! Visualizes money flow between sectors and detects rotation patterns.
//! Uses sector ETFs to estimate inter-sector capital flows.

pub mod etf_tracker;
pub mod rotation;
pub mod sector_flows;

pub use etf_tracker::{ETFPerformance, SectorETF, SectorETFTracker};
pub use rotation::{RotationDetector, RotationPattern, RotationType};
pub use sector_flows::{FlowMapData, SectorFlow, SectorNode};
