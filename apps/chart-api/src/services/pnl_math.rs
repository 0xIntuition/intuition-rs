use sqlx::types::BigDecimal;

pub fn compute_pnl_pct(total_pnl: &BigDecimal, net_invested: &BigDecimal) -> BigDecimal {
    let zero = BigDecimal::from(0);
    if net_invested > &zero {
        (total_pnl.clone() * BigDecimal::from(100)) / net_invested.clone()
    } else {
        zero
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_zero_when_net_invested_is_zero() {
        let pnl_pct = compute_pnl_pct(&BigDecimal::from(10), &BigDecimal::from(0));
        assert_eq!(pnl_pct, BigDecimal::from(0));
    }

    #[test]
    fn returns_zero_when_net_invested_is_negative() {
        let pnl_pct = compute_pnl_pct(&BigDecimal::from(10), &BigDecimal::from(-5));
        assert_eq!(pnl_pct, BigDecimal::from(0));
    }

    #[test]
    fn computes_percentage_when_net_invested_positive() {
        let pnl_pct = compute_pnl_pct(&BigDecimal::from(25), &BigDecimal::from(50));
        assert_eq!(pnl_pct, BigDecimal::from(50));
    }
}
