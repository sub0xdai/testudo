//! OODA types (placeholder)

#[derive(Debug, Clone)]
pub struct TradeIntent;
pub struct MarketObservation;
pub struct TradeSetup;
pub struct ExecutionPlan;

#[derive(Debug, Clone)]
pub enum OodaPhase {
    Observe,
    Orient,
    Decide,
    Act,
}

pub struct LoopMetrics;