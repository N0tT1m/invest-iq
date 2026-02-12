use rust_decimal::Decimal;

/// Compute the mark-to-market value of a short position.
///
/// Short value = entry_price * shares + (entry_price - current_price) * shares
/// Which simplifies to: 2 * entry_price * shares - current_price * shares
/// The first term is the initial cash received, the second is unrealized P&L.
pub fn short_position_mtm(
    entry_price: Decimal,
    current_price: Decimal,
    shares: Decimal,
) -> Decimal {
    // Cash received from short sale + unrealized gain/loss
    // If current_price < entry_price, this is positive (profit)
    // The notional value tied up is approximately entry_price * shares
    // MtM value = entry_price * shares + (entry_price - current_price) * shares
    (entry_price + entry_price - current_price) * shares
}

/// Compute P&L for closing a short position.
///
/// For shorts: P&L = (entry_price - exit_price) * shares (inverted from longs).
pub fn short_pnl(entry_price: Decimal, exit_price: Decimal, shares: Decimal) -> Decimal {
    (entry_price - exit_price) * shares
}

/// Compute fill price with directional slippage for a short entry.
///
/// Short entry (selling) fills BELOW the open price (adverse slippage for the seller).
pub fn short_entry_fill(open_price: Decimal, slippage_dec: Decimal) -> Decimal {
    open_price - open_price * slippage_dec
}

/// Compute fill price with directional slippage for a short exit (buy to cover).
///
/// Short exit (buying) fills ABOVE the open price (adverse slippage for the buyer).
pub fn short_exit_fill(raw_price: Decimal, slippage_dec: Decimal) -> Decimal {
    raw_price + raw_price * slippage_dec
}

/// Check stop-loss for a short position.
///
/// For shorts, stop-loss triggers when price goes ABOVE the stop level.
pub fn short_stop_loss_triggered(bar_high: Decimal, stop_loss_price: Decimal) -> bool {
    bar_high >= stop_loss_price
}

/// Check take-profit for a short position.
///
/// For shorts, take-profit triggers when price goes BELOW the take-profit level.
pub fn short_take_profit_triggered(bar_low: Decimal, take_profit_price: Decimal) -> bool {
    bar_low <= take_profit_price
}

/// Compute the gap-through fill price for a short stop-loss.
///
/// If bar opens above the stop, fill at the open (worse for the short).
pub fn short_sl_fill_price(bar_open: Decimal, stop_loss_price: Decimal) -> Decimal {
    if bar_open >= stop_loss_price {
        bar_open // Gap-through: fill at the worse open price
    } else {
        stop_loss_price
    }
}

/// Compute the gap-through fill price for a short take-profit.
///
/// If bar opens below the TP, fill at the open (better for the short).
pub fn short_tp_fill_price(bar_open: Decimal, take_profit_price: Decimal) -> Decimal {
    if bar_open <= take_profit_price {
        bar_open // Gap-through: fill at the better open price
    } else {
        take_profit_price
    }
}
