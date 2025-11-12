/// Pricing module for Numo Engine pools
/// Calculates marginal prices and solves for optimal trade sizes

use anyhow::Result;
use ethers::prelude::*;

use numo_bindings::NumoEnginePool;

/// Small amount for price discovery (1e15 = 0.001 base tokens with 18 decimals)
const PRICE_PROBE_AMOUNT: u128 = 1_000_000_000_000_000;

/// Maximum iterations for bisection solver
const MAX_BISECTION_ITERATIONS: usize = 25;

/// Pool state snapshot
#[derive(Debug, Clone)]
pub struct PoolState {
    pub address: Address,
    pub base_reserves: u128,
    pub fy_reserves: u128,
    pub fee_bps: u16,
    pub maturity: u32,
}

/// Calculate marginal price (base per FY) for a pool
/// Uses small buy and sell previews to estimate the mid-price
pub async fn marginal_price_base_per_fy<M: Middleware + 'static>(
    pool: &NumoEnginePool<M>,
) -> Result<U256> {
    // Probe 1: Sell small amount of base, see how much FY we get
    // Price1 = base_in / fy_out (base per FY)
    let fy_out = pool.sell_base_preview(PRICE_PROBE_AMOUNT).call().await?;
    let fy_out = fy_out.max(1); // Avoid division by zero

    let one_e18 = U256::exp10(18);
    let price1 = U256::from(PRICE_PROBE_AMOUNT) * one_e18 / U256::from(fy_out);

    // Probe 2: Sell small amount of FY, see how much base we get
    // Price2 = base_out / fy_in (base per FY)
    let base_out = pool.sell_fy_token_preview(PRICE_PROBE_AMOUNT).call().await?;
    let base_out = base_out.max(1);

    let price2 = U256::from(base_out) * one_e18 / U256::from(PRICE_PROBE_AMOUNT);

    // Return average of bid and ask
    Ok((price1 + price2) / U256::from(2))
}

/// Get pool state (reserves, fees, maturity)
pub async fn get_pool_state<M: Middleware + 'static>(
    pool: &NumoEnginePool<M>,
    address: Address,
) -> Result<PoolState> {
    let (base_reserves, fy_reserves, fee_bps) = pool.get_cache().call().await?;
    let maturity = pool.maturity().call().await?;

    Ok(PoolState {
        address,
        base_reserves,
        fy_reserves,
        fee_bps,
        maturity,
    })
}

/// Solve for the amount of FY tokens to trade such that the post-trade
/// marginal price of the rich pool equals the target price
///
/// This uses a simple bisection search. For production, consider implementing
/// a local Numo Engine quoter that computes exact post-trade prices from the
/// constant-product formula.
pub async fn solve_fy_amount_to_target<M: Middleware + Clone + 'static>(
    rich_pool: &NumoEnginePool<M>,
    target_price_1e18: U256,
    max_fy_amount: u128,
) -> Result<Option<u128>> {
    let mut lo: u128 = 0;
    let mut hi: u128 = max_fy_amount;
    let mut best: u128 = 0;

    for iteration in 0..MAX_BISECTION_ITERATIONS {
        if hi <= lo {
            break;
        }

        let mid = ((U256::from(lo) + U256::from(hi)) / U256::from(2)).as_u128();
        if mid == 0 {
            break;
        }

        // Get current marginal price as proxy for post-trade price
        // In production, compute exact post-trade price from updated reserves
        let current_price = marginal_price_base_per_fy(rich_pool).await?;

        tracing::debug!(
            iteration,
            mid,
            current_price = %current_price,
            target_price = %target_price_1e18,
            "Bisection iteration"
        );

        // If current price is higher than target, we need to sell more FY (push price down)
        if current_price > target_price_1e18 {
            best = mid;
            lo = mid.saturating_add(1);
        } else {
            // Price is already at or below target
            hi = mid.saturating_sub(1);
        }

        // Check convergence
        if hi - lo < 1000 {
            break;
        }
    }

    if best == 0 {
        Ok(None)
    } else {
        Ok(Some(best))
    }
}

/// Calculate expected profit from an arbitrage trade
/// Returns (gross_profit, net_profit) in base token units
pub fn calculate_profit(
    base_spent: u128,
    base_received: u128,
    estimated_gas_cost: u128,
) -> (u128, i128) {
    let gross_profit = base_received.saturating_sub(base_spent);
    let net_profit = (gross_profit as i128) - (estimated_gas_cost as i128);
    (gross_profit, net_profit)
}

/// Calculate slippage-adjusted amounts
/// Adds slippage_bps to maxIn, subtracts from minOut
pub fn apply_slippage(amount: u128, slippage_bps: u32, is_max_in: bool) -> u128 {
    let adjustment = (amount as u128 * slippage_bps as u128) / 10_000;

    if is_max_in {
        // For max_in, add slippage buffer
        amount.saturating_add(adjustment)
    } else {
        // For min_out, subtract slippage buffer
        amount.saturating_sub(adjustment)
    }
}

/// Calculate price divergence in basis points
/// Returns how many basis points the pool price differs from target
pub fn price_divergence_bps(pool_price: U256, target_price: U256) -> u32 {
    if target_price.is_zero() {
        return 0;
    }

    let diff = if pool_price > target_price {
        pool_price - target_price
    } else {
        target_price - pool_price
    };

    let divergence = (diff * U256::from(10_000)) / target_price;
    divergence.as_u32()
}

/// Check if arbitrage opportunity meets minimum edge threshold
pub fn meets_edge_threshold(
    pool_price: U256,
    target_price: U256,
    edge_bps: u32,
) -> bool {
    let divergence = price_divergence_bps(pool_price, target_price);
    divergence >= edge_bps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_profit() {
        // Profitable trade
        let (gross, net) = calculate_profit(1000, 1100, 20);
        assert_eq!(gross, 100);
        assert_eq!(net, 80);

        // Unprofitable after gas
        let (gross, net) = calculate_profit(1000, 1050, 100);
        assert_eq!(gross, 50);
        assert_eq!(net, -50);
    }

    #[test]
    fn test_apply_slippage() {
        // 1% slippage = 100 bps
        let amount = 10_000u128;

        // Max in - add slippage
        let max_in = apply_slippage(amount, 100, true);
        assert_eq!(max_in, 10_100);

        // Min out - subtract slippage
        let min_out = apply_slippage(amount, 100, false);
        assert_eq!(min_out, 9_900);
    }

    #[test]
    fn test_price_divergence_bps() {
        let target = U256::from(1_000_000);

        // 1% higher
        let pool_high = U256::from(1_010_000);
        assert_eq!(price_divergence_bps(pool_high, target), 100); // 100 bps

        // 0.5% lower
        let pool_low = U256::from(995_000);
        assert_eq!(price_divergence_bps(pool_low, target), 50); // 50 bps
    }

    #[test]
    fn test_meets_edge_threshold() {
        let target = U256::from(1_000_000);

        // 15 bps edge required
        let edge_bps = 15;

        // 10 bps divergence - doesn't meet threshold
        let pool1 = U256::from(1_001_000);
        assert!(!meets_edge_threshold(pool1, target, edge_bps));

        // 20 bps divergence - meets threshold
        let pool2 = U256::from(1_002_000);
        assert!(meets_edge_threshold(pool2, target, edge_bps));
    }
}
