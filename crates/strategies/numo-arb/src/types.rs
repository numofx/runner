/// Types for the Numo arbitrage strategy

use ethers::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export types from artemis_core
pub use artemis_core::executors::mempool_executor::{GasBidInfo, SubmitTxToMempool};

/// Configuration for the Numo arbitrage strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Address of the deployed NumoArbRouter contract
    pub router_address: Address,

    /// List of Numo Engine pool addresses to monitor
    pub pool_addresses: Vec<Address>,

    /// Minimum edge in basis points before executing arb (e.g., 10 = 0.10%)
    pub edge_bps: u32,

    /// Slippage tolerance in basis points (e.g., 50 = 0.50%)
    pub slippage_bps: u32,

    /// Maximum FY token amount to trade per transaction (in smallest units)
    pub max_fy_amount: u128,

    /// Maximum base token amount to risk per transaction
    pub max_base_amount: u128,

    /// Percentage of expected profit to bid in gas (0-100)
    pub bid_percentage: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            router_address: Address::zero(),
            pool_addresses: vec![],
            edge_bps: 10,          // 0.10% minimum edge
            slippage_bps: 50,      // 0.50% slippage tolerance
            max_fy_amount: 100_000u128 * 10u128.pow(18), // 100k tokens
            max_base_amount: 50_000u128 * 10u128.pow(18), // 50k tokens
            bid_percentage: 80,    // Bid 80% of profit in gas
        }
    }
}

/// Events that the Numo strategy processes
#[derive(Debug, Clone)]
pub enum Event {
    /// New block event with timestamp
    NewBlock(NewBlockEvent),
}

#[derive(Debug, Clone)]
pub struct NewBlockEvent {
    pub block_number: u64,
    pub timestamp: u64,
    pub base_fee: Option<U256>,
}

/// Actions that the Numo strategy can emit
#[derive(Debug, Clone)]
pub enum Action {
    /// Submit a transaction to the mempool
    SubmitTx(SubmitTxToMempool),
}

/// Arbitrage opportunity details
#[derive(Debug, Clone)]
pub struct ArbOpportunity {
    pub cheap_pool: Address,
    pub rich_pool: Address,
    pub fy_amount: u128,
    pub max_base_in: u128,
    pub min_base_out: u128,
    pub expected_profit: u128,
    pub target_price: U256,
    pub cheap_price: U256,
    pub rich_price: U256,
}

impl ArbOpportunity {
    /// Check if the opportunity is still profitable after gas costs
    pub fn is_profitable(&self, gas_cost: u128) -> bool {
        self.expected_profit > gas_cost
    }

    /// Calculate net profit after gas costs
    pub fn net_profit(&self, gas_cost: u128) -> i128 {
        (self.expected_profit as i128) - (gas_cost as i128)
    }
}

