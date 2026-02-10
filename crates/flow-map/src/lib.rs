//! Flow Map Module
//!
//! Visualizes money flow between sectors and detects rotation patterns.
//! Uses sector ETFs to estimate inter-sector capital flows.

pub mod sector_flows;
pub mod etf_tracker;
pub mod rotation;

pub use sector_flows::{FlowMapData, SectorFlow, SectorNode};
pub use etf_tracker::{SectorETF, ETFPerformance, SectorETFTracker};
pub use rotation::{RotationPattern, RotationDetector, RotationType};
