/// Main strategy module for Numo Engine arbitrage

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use anyhow::Result;
use ethers::prelude::*;
use tracing::{debug, info, warn};

use artemis_core::types::Strategy;

use numo_bindings::{NumoArbRouter, NumoEnginePool};
use crate::pricing::{
    apply_slippage, get_pool_state, marginal_price_base_per_fy, meets_edge_threshold,
    solve_fy_amount_to_target, PoolState,
};
use crate::sofr::SofrCurve;
use crate::types::{Action, ArbOpportunity, Config, Event, GasBidInfo, NewBlockEvent, SubmitTxToMempool};

/// Numo arbitrage strategy
/// Monitors Numo Engine pools and executes arbitrage when prices diverge from SOFR curve
pub struct NumoArb<M: Middleware> {
    /// Ethereum client
    client: Arc<M>,

    /// Strategy configuration
    config: Config,

    /// SOFR curve for discount factor calculations
    sofr_curve: SofrCurve,

    /// Router contract instance
    router: NumoArbRouter<M>,

    /// Pool state cache
    pool_states: HashMap<Address, PoolState>,

    /// Last processed block
    last_block: u64,
}

impl<M: Middleware + Clone + 'static> NumoArb<M> {
    /// Create a new Numo arbitrage strategy
    pub fn new(
        client: Arc<M>,
        config: Config,
        sofr_curve: SofrCurve,
    ) -> Self {
        let router = NumoArbRouter::new(config.router_address, client.clone());

        Self {
            client,
            config,
            sofr_curve,
            router,
            pool_states: HashMap::new(),
            last_block: 0,
        }
    }

    /// Find the best arbitrage opportunity between pools
    async fn find_best_opportunity(
        &self,
        current_ts: u64,
    ) -> Result<Option<ArbOpportunity>> {
        if self.pool_states.len() < 2 {
            return Ok(None);
        }

        let mut best_opp: Option<ArbOpportunity> = None;
        let mut max_profit: u128 = 0;

        // Get prices for all pools
        let mut pool_prices: Vec<(Address, U256, f64)> = Vec::new();

        for pool_addr in &self.config.pool_addresses {
            let pool = NumoEnginePool::new(*pool_addr, self.client.clone());

            match marginal_price_base_per_fy(&pool).await {
                Ok(price) => {
                    if let Some(state) = self.pool_states.get(pool_addr) {
                        let ttm = self.sofr_curve.time_to_maturity(current_ts, state.maturity);
                        pool_prices.push((*pool_addr, price, ttm));
                    }
                }
                Err(e) => {
                    warn!(pool = ?pool_addr, error = ?e, "Failed to get pool price");
                }
            }
        }

        // Find cheap and rich pools
        // Cheap = lowest price (FY is undervalued)
        // Rich = highest price (FY is overvalued)
        if pool_prices.is_empty() {
            return Ok(None);
        }

        let cheap_idx = match pool_prices
            .iter()
            .enumerate()
            .min_by_key(|(_, (_, price, _))| *price)
            .map(|(i, _)| i)
        {
            Some(idx) => idx,
            None => return Ok(None),
        };

        let rich_idx = match pool_prices
            .iter()
            .enumerate()
            .max_by_key(|(_, (_, price, _))| *price)
            .map(|(i, _)| i)
        {
            Some(idx) => idx,
            None => return Ok(None),
        };

        if cheap_idx == rich_idx {
            return Ok(None);
        }

        let (cheap_addr, cheap_price, _) = pool_prices[cheap_idx];
        let (rich_addr, rich_price, ttm_rich) = pool_prices[rich_idx];

        // Calculate target price from SOFR
        let target_df = self.sofr_curve.discount_factor(ttm_rich);
        let target_price = U256::from((target_df * 1e18) as u128);

        debug!(
            cheap_pool = ?cheap_addr,
            rich_pool = ?rich_addr,
            cheap_price = %cheap_price,
            rich_price = %rich_price,
            target_price = %target_price,
            "Found potential opportunity"
        );

        // Check if rich pool price is high enough above target
        if !meets_edge_threshold(rich_price, target_price, self.config.edge_bps) {
            debug!("Opportunity doesn't meet edge threshold");
            return Ok(None);
        }

        // Solve for optimal FY amount to trade
        let rich_pool = NumoEnginePool::new(rich_addr, self.client.clone());
        let fy_amount = solve_fy_amount_to_target(
            &rich_pool,
            target_price,
            self.config.max_fy_amount,
        )
        .await?;

        let fy_amount = match fy_amount {
            Some(amt) if amt > 0 => amt,
            _ => {
                debug!("Could not solve for FY amount");
                return Ok(None);
            }
        };

        // Calculate expected costs and returns
        let cheap_pool = NumoEnginePool::new(cheap_addr, self.client.clone());
        let max_base_in = cheap_pool.buy_fy_token_preview(fy_amount).call().await?;
        let min_base_out = rich_pool.sell_fy_token_preview(fy_amount).call().await?;

        if max_base_in >= min_base_out {
            debug!("Trade would be unprofitable before slippage");
            return Ok(None);
        }

        let expected_profit = min_base_out.saturating_sub(max_base_in);

        // Apply slippage protection
        let max_base_in_slip = apply_slippage(max_base_in, self.config.slippage_bps, true);
        let min_base_out_slip = apply_slippage(min_base_out, self.config.slippage_bps, false);

        // Check we're not exceeding position limits
        if max_base_in_slip > self.config.max_base_amount {
            warn!(
                max_base_in = max_base_in_slip,
                limit = self.config.max_base_amount,
                "Trade exceeds max base amount"
            );
            return Ok(None);
        }

        let opportunity = ArbOpportunity {
            cheap_pool: cheap_addr,
            rich_pool: rich_addr,
            fy_amount,
            max_base_in: max_base_in_slip,
            min_base_out: min_base_out_slip,
            expected_profit,
            target_price,
            cheap_price,
            rich_price,
        };

        if expected_profit > max_profit {
            max_profit = expected_profit;
            best_opp = Some(opportunity);
        }

        Ok(best_opp)
    }

    /// Execute an arbitrage opportunity
    async fn execute_arbitrage(&self, opp: ArbOpportunity) -> Result<Option<Action>> {
        info!(
            cheap_pool = ?opp.cheap_pool,
            rich_pool = ?opp.rich_pool,
            fy_amount = opp.fy_amount,
            expected_profit = opp.expected_profit,
            "Executing arbitrage"
        );

        // Build transaction to call router
        let call = self.router.arb_buy_fy_then_sell_fy(
            opp.cheap_pool,
            opp.rich_pool,
            opp.fy_amount,
            opp.max_base_in,
            opp.min_base_out,
            self.client.default_sender().unwrap_or_default(),
        );

        // Estimate gas
        let gas_estimate = call.estimate_gas().await.unwrap_or(U256::from(500_000));
        let gas_with_buffer = gas_estimate * U256::from(120) / U256::from(100); // 20% buffer

        // Build transaction
        let mut tx = call.tx;
        tx.set_gas(gas_with_buffer);

        // Create gas bid info
        let gas_bid_info = Some(GasBidInfo {
            total_profit: U256::from(opp.expected_profit),
            bid_percentage: self.config.bid_percentage,
        });

        let action = Action::SubmitTx(SubmitTxToMempool {
            tx,
            gas_bid_info,
        });

        Ok(Some(action))
    }

    /// Process a new block event
    async fn process_new_block(&mut self, block: NewBlockEvent) -> Vec<Action> {
        self.last_block = block.block_number;

        debug!(block_number = block.block_number, "Processing new block");

        // Find arbitrage opportunity
        let opportunity = match self.find_best_opportunity(block.timestamp).await {
            Ok(Some(opp)) => opp,
            Ok(None) => {
                debug!("No profitable opportunity found");
                return vec![];
            }
            Err(e) => {
                warn!(error = ?e, "Error finding opportunity");
                return vec![];
            }
        };

        // Execute if profitable
        match self.execute_arbitrage(opportunity).await {
            Ok(Some(action)) => vec![action],
            Ok(None) => vec![],
            Err(e) => {
                warn!(error = ?e, "Error executing arbitrage");
                vec![]
            }
        }
    }
}

#[async_trait]
impl<M: Middleware + Clone + 'static> Strategy<Event, Action> for NumoArb<M> {
    async fn sync_state(&mut self) -> Result<()> {
        info!("Syncing Numo strategy state");

        // Fetch initial state for all pools
        for pool_addr in &self.config.pool_addresses {
            let pool = NumoEnginePool::new(*pool_addr, self.client.clone());

            match get_pool_state(&pool, *pool_addr).await {
                Ok(state) => {
                    info!(
                        pool = ?pool_addr,
                        base_reserves = state.base_reserves,
                        fy_reserves = state.fy_reserves,
                        maturity = state.maturity,
                        "Loaded pool state"
                    );
                    self.pool_states.insert(*pool_addr, state);
                }
                Err(e) => {
                    warn!(pool = ?pool_addr, error = ?e, "Failed to load pool state");
                }
            }
        }

        info!(pools_loaded = self.pool_states.len(), "State sync complete");
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::NewBlock(block) => self.process_new_block(block).await,
        }
    }
}
