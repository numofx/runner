/// SOFR (Secured Overnight Financing Rate) curve module
/// Implements discount factor calculations using ACT/360 day count convention

use serde::{Deserialize, Serialize};

/// Day count convention for fixed income calculations
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DayCount {
    /// Actual/360 - commonly used for USD money market instruments
    Act360,
    /// Actual/365 - alternative convention
    Act365,
}

impl DayCount {
    /// Convert days to year fraction
    pub fn year_fraction(&self, days: i64) -> f64 {
        match self {
            DayCount::Act360 => days as f64 / 360.0,
            DayCount::Act365 => days as f64 / 365.0,
        }
    }

    /// Convert seconds to year fraction (for blockchain timestamps)
    pub fn year_fraction_from_seconds(&self, seconds: i64) -> f64 {
        let days = seconds as f64 / 86400.0; // seconds per day
        match self {
            DayCount::Act360 => days / 360.0,
            DayCount::Act365 => days / 365.0,
        }
    }
}

/// A knot point on the SOFR curve
/// Represents (time_to_maturity_years, simple_rate)
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CurveKnot {
    /// Time to maturity in years (ACT/360)
    pub t: f64,
    /// Simple interest rate (e.g., 0.0520 = 5.20%)
    pub rate: f64,
}

/// SOFR discount factor curve
/// Uses piecewise-linear interpolation in simple rate space
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SofrCurve {
    /// Curve knot points, must be sorted by time
    pub knots: Vec<CurveKnot>,
    /// Day count convention
    pub day_count: DayCount,
}

impl SofrCurve {
    /// Create a new SOFR curve with given knots
    /// Knots should be sorted by time
    pub fn new(knots: Vec<CurveKnot>, day_count: DayCount) -> Self {
        Self { knots, day_count }
    }

    /// Create a default curve with sample SOFR rates
    /// These are placeholder values - replace with real market data
    pub fn default_usd() -> Self {
        Self {
            knots: vec![
                CurveKnot { t: 0.0028, rate: 0.0520 }, // ~1 day
                CurveKnot { t: 0.0833, rate: 0.0515 }, // 1 month
                CurveKnot { t: 0.25, rate: 0.0500 },   // 3 months
                CurveKnot { t: 0.50, rate: 0.0475 },   // 6 months
                CurveKnot { t: 1.00, rate: 0.0450 },   // 1 year
                CurveKnot { t: 2.00, rate: 0.0425 },   // 2 years
            ],
            day_count: DayCount::Act360,
        }
    }

    /// Calculate discount factor for a given time to maturity
    /// DF(t) = 1 / (1 + r(t) * t) using simple compounding
    pub fn discount_factor(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 1.0;
        }
        let rate = self.interpolate_rate(t);
        1.0 / (1.0 + rate * t)
    }

    /// Calculate implied forward rate between two times
    /// F(t1, t2) = [DF(t1) / DF(t2) - 1] / (t2 - t1)
    pub fn forward_rate(&self, t1: f64, t2: f64) -> f64 {
        if t2 <= t1 {
            return 0.0;
        }
        let df1 = self.discount_factor(t1);
        let df2 = self.discount_factor(t2);
        (df1 / df2 - 1.0) / (t2 - t1)
    }

    /// Interpolate simple rate for a given time using piecewise-linear method
    fn interpolate_rate(&self, t: f64) -> f64 {
        let n = self.knots.len();
        if n == 0 {
            return 0.0;
        }

        // Before first knot - use first rate
        if t <= self.knots[0].t {
            return self.knots[0].rate;
        }

        // After last knot - use last rate (flat extrapolation)
        if t >= self.knots[n - 1].t {
            return self.knots[n - 1].rate;
        }

        // Linear interpolation between knots
        for i in 1..n {
            let (t0, r0) = (self.knots[i - 1].t, self.knots[i - 1].rate);
            let (t1, r1) = (self.knots[i].t, self.knots[i].rate);

            if t <= t1 {
                // Linear interpolation: r = r0 + (r1 - r0) * (t - t0) / (t1 - t0)
                let alpha = (t - t0) / (t1 - t0);
                return r0 + alpha * (r1 - r0);
            }
        }

        // Should never reach here
        self.knots[n - 1].rate
    }

    /// Get rate for a given time (alias for interpolate_rate)
    pub fn rate(&self, t: f64) -> f64 {
        self.interpolate_rate(t)
    }

    /// Calculate time to maturity from current timestamp and maturity timestamp
    pub fn time_to_maturity(&self, current_ts: u64, maturity_ts: u32) -> f64 {
        let seconds_to_maturity = (maturity_ts as i64 - current_ts as i64).max(0);
        self.day_count.year_fraction_from_seconds(seconds_to_maturity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discount_factor() {
        let curve = SofrCurve::default_usd();

        // DF at t=0 should be 1.0
        assert!((curve.discount_factor(0.0) - 1.0).abs() < 1e-10);

        // DF should decrease with time
        let df1 = curve.discount_factor(0.5);
        let df2 = curve.discount_factor(1.0);
        assert!(df1 > df2);

        // DF should be less than 1 for positive time
        assert!(df1 < 1.0);
        assert!(df2 < 1.0);
    }

    #[test]
    fn test_rate_interpolation() {
        let curve = SofrCurve::new(
            vec![
                CurveKnot { t: 0.0, rate: 0.05 },
                CurveKnot { t: 1.0, rate: 0.04 },
            ],
            DayCount::Act360,
        );

        // At knot points
        assert!((curve.rate(0.0) - 0.05).abs() < 1e-10);
        assert!((curve.rate(1.0) - 0.04).abs() < 1e-10);

        // Midpoint should be average
        let mid_rate = curve.rate(0.5);
        assert!((mid_rate - 0.045).abs() < 1e-10);
    }

    #[test]
    fn test_time_to_maturity() {
        let curve = SofrCurve::default_usd();

        // 365 days from now using Act360
        let current = 1700000000u64; // arbitrary timestamp
        let maturity = (current + 365 * 86400) as u32; // +365 days

        let ttm = curve.time_to_maturity(current, maturity);
        // Should be approximately 1.0139 years in Act/360
        assert!((ttm - 1.0139).abs() < 0.001);
    }

    #[test]
    fn test_forward_rate() {
        let curve = SofrCurve::default_usd();

        // Forward rate should be positive
        let fwd = curve.forward_rate(0.5, 1.0);
        assert!(fwd > 0.0);

        // Forward rate (0, t) should equal spot rate
        let spot = curve.rate(0.5);
        let fwd_from_zero = curve.forward_rate(0.0, 0.5);
        // They won't be exactly equal due to simple vs. forward compounding
        // but should be close
        assert!((fwd_from_zero - spot).abs() < 0.01);
    }
}
