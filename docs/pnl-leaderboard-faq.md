# PnL Leaderboard FAQ

## How does the leaderboard work?

The PnL leaderboard measures trading performance within a specific **epoch** (time window). It tracks how well you deployed capital during that period, breaking your performance into **realized PnL** (profit you locked in by redeeming) and **unrealized PnL** (paper gains/losses on positions you still hold).

---

## Key Concepts

### What counts as "realized PnL"?

Realized PnL is profit (or loss) that you have **locked in by redeeming shares** during the epoch. Crucially, the leaderboard only counts realized PnL on capital that was **deployed during the current epoch**.

If you deposited before the epoch started and redeemed during it, that profit does not count toward your realized PnL for this epoch. This is by design -- the leaderboard measures how well you traded **within** the epoch window, not how well you timed a position that was opened weeks ago.

### What counts as "unrealized PnL"?

Unrealized PnL is the change in the value of your open positions over the epoch. It is calculated as:

```
Unrealized PnL = (Equity at End - Equity at Start + Redemptions - Deposits) - Realized PnL
```

This captures both the appreciation of positions you held into the epoch and positions you opened during it.

### What is "total PnL"?

Total PnL = Realized PnL + Unrealized PnL. This is the overall change in your portfolio value during the epoch, accounting for all deposits and withdrawals.

---

## Common Questions

### "I redeemed 100 TRUST in profit but my realized PnL shows 0"

This is the most common point of confusion. If you **deposited before the epoch** and **redeemed during it**, your realized PnL for this epoch is 0 -- even if you withdrew more than you put in.

**Why?** The leaderboard is epoch-scoped. It measures skill demonstrated *during this epoch*. Capital deployed before the epoch is considered pre-existing -- the decision to enter that position happened in a prior period. Your profit from that position will still show up in your **unrealized PnL** (the change in equity value) and in your **total PnL**.

**Example:**
- Before the epoch: You deposited 50 TRUST into a claim
- During the epoch: That position grew and you redeemed 100 TRUST
- Realized PnL: **0** (no in-epoch capital was deployed and closed)
- Unrealized PnL: **~50 TRUST** (the position value changed during the epoch)
- Total PnL: **~50 TRUST**

### "I deposited AND redeemed during this epoch but my realized PnL seems low"

When you have both pre-existing shares (from before the epoch) and new deposits within the epoch, the leaderboard uses **FIFO (First-In-First-Out) accounting**. Pre-existing shares are consumed first when you redeem.

**Example:**
- Before the epoch: You hold 10 shares
- During the epoch: You deposit 5 more shares, then redeem 12 shares
- FIFO consumes your 10 pre-existing shares first, then 2 of your in-epoch shares
- Realized PnL is only calculated on the **2 in-epoch shares** that were redeemed
- The profit from redeeming the 10 pre-existing shares counts toward unrealized PnL

### "My numbers don't match what I see in my transaction history"

There are a few reasons the numbers might look different from what you expect:

1. **Epoch scoping**: The leaderboard only considers activity within the epoch window. Deposits or redemptions outside that window are not counted.

2. **Fees**: Redemption fees are deducted from the amount you receive. The leaderboard uses post-fee amounts.

3. **Bonding curve pricing**: For curve 2 (bonding curve) vaults, equity is calculated using the bonding curve formula rather than a simple `shares x price` calculation. This accounts for price impact and gives a more accurate picture of what your shares are actually worth if redeemed.

4. **Hourly granularity**: The leaderboard uses hourly snapshots for position tracking. Activity within the same hour is aggregated together.

### "Why am I not on the leaderboard at all?"

You need to meet the minimum thresholds to appear:

- **Minimum deposit**: Positions where your all-time cumulative deposits are below the threshold (e.g., 5 TRUST) are excluded. This prevents gaming with dust amounts across many positions.
- **Minimum positions**: You need at least 1 active position with activity during the epoch (default).
- **Account type**: Protocol vaults and atom wallets are excluded by default.

### "What is the minimum deposit filter?"

The minimum deposit filter (`p_min_deposit`) excludes positions where your **cumulative all-time deposits** are below a set threshold. For example, with a 5 TRUST threshold, any position where you have deposited less than 5 TRUST total (across all time, not just this epoch) is completely excluded from all leaderboard calculations.

This applies per-position, not per-account. If you have 10 positions but 3 of them are below the threshold, only the 7 qualifying positions count toward your leaderboard stats.

### "What is the difference between Total PnL ranking and Realized PnL ranking?"

- **Total PnL ranking**: Ranks by overall portfolio performance including paper gains. Good for measuring who picked the best positions overall.
- **Realized PnL ranking**: Ranks by profit that was actually locked in through redemptions of in-epoch capital. Good for measuring active trading skill.
- **PnL % ranking**: Ranks by return on capital deployed. Good for comparing efficiency regardless of portfolio size.

### "My position went up a lot but my PnL % is low"

PnL % is calculated as:

```
PnL % = Total PnL / (Equity at Start + Deposits during Epoch)
```

If you had a large existing portfolio at the start of the epoch, your denominator is large, which dilutes the percentage even if individual positions performed well. This is intentional -- it measures return on total capital at risk.

---

## How Realized PnL is Calculated (Technical)

The calculation depends on the type of position:

| Scenario | Realized PnL Formula |
|----------|---------------------|
| No redemptions during epoch | 0 |
| No deposits during epoch (pre-existing position only) | 0 |
| New position (no pre-existing shares) | Redemption proceeds - Pro-rata cost of redeemed shares |
| Mixed position (pre-existing + new deposits + redemptions) | FIFO accounting (see below) |

### FIFO Accounting for Mixed Positions

When you have pre-existing shares AND deposit new capital AND redeem during the same epoch, the system processes events chronologically hour by hour:

1. **Pre-existing shares are consumed first** on any redemption (FIFO order)
2. **In-epoch shares are consumed second** after all pre-existing shares are gone
3. **Realized PnL only comes from in-epoch shares** that are redeemed
4. For each hour's redemption, the formula is:
   - Proceeds from in-epoch shares = `(redemption amount) x (in-epoch shares redeemed / total shares redeemed)`
   - Cost basis of in-epoch shares = `(accumulated epoch cost) x (in-epoch shares redeemed / total in-epoch shares)`
   - Realized PnL = Proceeds - Cost basis

This ensures that profit from pre-existing positions does not inflate the epoch's realized PnL metric.

---

## Glossary

| Term | Definition |
|------|-----------|
| **Epoch** | The time window for the leaderboard (e.g., Apr 7 12:00 PM - Apr 21 12:00 PM) |
| **Realized PnL** | Profit locked in by redeeming in-epoch capital during the epoch |
| **Unrealized PnL** | Paper gain/loss on positions still open at epoch end |
| **Total PnL** | Realized + Unrealized PnL |
| **Equity at Start** | Market value of your positions at epoch start |
| **Equity at End** | Market value of your positions at epoch end |
| **FIFO** | First-In-First-Out: pre-existing shares are redeemed before in-epoch shares |
| **Minimum Deposit** | Per-position threshold; positions below this cumulative deposit amount are excluded |
| **Active Position** | A position where you still hold shares at epoch end |
| **Period Position** | A position where you had any deposit or redemption activity during the epoch |
