//! Tax Optimizer
//!
//! Automatic tax-loss harvesting suggestions with wash sale rule monitoring.
//! Supports multiple tax jurisdictions (US, UK, Canada, Australia, Germany).

pub mod harvester;
pub mod wash_sale;
pub mod substitutes;
pub mod tax_calculator;

pub use harvester::{
    HarvestOpportunity, HarvestingEngine, HarvestResult, HarvestPriority,
    HarvestingConfig, HarvestSummary,
};
pub use wash_sale::{
    WashSaleMonitor, WashSaleViolation, WashSaleWindow, WashSaleStatus,
    WashSaleCalendar, WashSaleSummary,
};
pub use substitutes::{
    SubstituteFinder, SubstituteSecurity, SubstituteType, CorrelationScore,
};
pub use tax_calculator::{
    TaxCalculator, TaxJurisdiction, TaxRules, TaxEstimate, TaxLot,
    HoldingPeriod, GainType, YearEndSummary,
};
