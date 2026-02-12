//! Tax Optimizer
//!
//! Automatic tax-loss harvesting suggestions with wash sale rule monitoring.
//! Supports multiple tax jurisdictions (US, UK, Canada, Australia, Germany).

pub mod harvester;
pub mod substitutes;
pub mod tax_calculator;
pub mod wash_sale;

pub use harvester::{
    HarvestOpportunity, HarvestPriority, HarvestResult, HarvestSummary, HarvestingConfig,
    HarvestingEngine,
};
pub use substitutes::{CorrelationScore, SubstituteFinder, SubstituteSecurity, SubstituteType};
pub use tax_calculator::{
    GainType, HoldingPeriod, TaxCalculator, TaxEstimate, TaxJurisdiction, TaxLot, TaxRules,
    YearEndSummary,
};
pub use wash_sale::{
    WashSaleCalendar, WashSaleMonitor, WashSaleStatus, WashSaleSummary, WashSaleViolation,
    WashSaleWindow,
};
