/// Numo Engine Arbitrage Strategy
///
/// This strategy monitors Numo Engine pools on Celo and executes arbitrage
/// when pool-implied discount factors diverge from the SOFR curve.
///
/// ## Architecture
///
/// - **Collector**: Monitors new blocks and triggers strategy evaluation
/// - **Strategy**: Compares pool prices to SOFR discount factors and finds opportunities
/// - **Executor**: Submits transactions via the NumoArbRouter contract
///
/// ## Key Components
///
/// - `sofr`: SOFR curve implementation for discount factor calculations
/// - `pricing`: Pool price discovery and trade sizing logic
/// - `strategy`: Main arbitrage strategy implementation
/// - `types`: Type definitions for events, actions, and configuration
/// - `bindings`: Contract ABI bindings for Numo Engine pools and router (external crate)
pub mod pricing;
pub mod sofr;
pub mod strategy;
pub mod types;

// Re-exports for convenience
pub use strategy::NumoArb;
pub use types::{Action, Config, Event};
