use analysis_core::SignalStrength;
use polygon_client::SnapshotTicker;
use serenity::builder::CreateEmbed;
use serenity::model::Timestamp;

const COLOR_GREEN: u32 = 0x00FF00;
const COLOR_RED: u32 = 0xFF0000;
const COLOR_GOLD: u32 = 0xFFD700;
const COLOR_BLUE: u32 = 0x3498DB;

fn signal_color(signal: &SignalStrength) -> u32 {
    match signal {
        SignalStrength::StrongBuy | SignalStrength::Buy | SignalStrength::WeakBuy => COLOR_GREEN,
        SignalStrength::Neutral => COLOR_GOLD,
        SignalStrength::WeakSell | SignalStrength::Sell | SignalStrength::StrongSell => COLOR_RED,
    }
}

fn signal_emoji(signal: &SignalStrength) -> &'static str {
    match signal {
        SignalStrength::StrongBuy => "\u{1F680}",
        SignalStrength::Buy => "\u{1F4C8}",
        SignalStrength::WeakBuy => "\u{2197}\u{FE0F}",
        SignalStrength::Neutral => "\u{27A1}\u{FE0F}",
        SignalStrength::WeakSell => "\u{2198}\u{FE0F}",
        SignalStrength::Sell => "\u{1F4C9}",
        SignalStrength::StrongSell => "\u{26A0}\u{FE0F}",
    }
}

fn parse_signal(s: &str) -> SignalStrength {
    match s {
        "StrongBuy" => SignalStrength::StrongBuy,
        "Buy" => SignalStrength::Buy,
        "WeakBuy" => SignalStrength::WeakBuy,
        "WeakSell" => SignalStrength::WeakSell,
        "Sell" => SignalStrength::Sell,
        "StrongSell" => SignalStrength::StrongSell,
        _ => SignalStrength::Neutral,
    }
}

fn footer() -> serenity::builder::CreateEmbedFooter {
    serenity::builder::CreateEmbedFooter::new("InvestIQ | Powered by Polygon.io")
}

fn footer_plain() -> serenity::builder::CreateEmbedFooter {
    serenity::builder::CreateEmbedFooter::new("InvestIQ")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn format_volume(v: f64) -> String {
    if v >= 1_000_000_000.0 {
        format!("{:.1}B", v / 1_000_000_000.0)
    } else if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.1}K", v / 1_000.0)
    } else {
        format!("{:.0}", v)
    }
}

pub fn parse_f64(val: &serde_json::Value, key: &str) -> f64 {
    val.get(key)
        .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(0.0)
}

fn risk_indicator(score: f64) -> &'static str {
    if score >= 70.0 { "\u{1F534}" }       // red circle
    else if score >= 40.0 { "\u{1F7E1}" }  // yellow circle
    else { "\u{1F7E2}" }                    // green circle
}

fn pnl_color(val: f64) -> u32 {
    if val >= 0.0 { COLOR_GREEN } else { COLOR_RED }
}

// ══════════════════════════════════════════════════════════════════════
// Existing embeds
// ══════════════════════════════════════════════════════════════════════

pub fn build_analysis_embed_from_json(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let signal = parse_signal(data.get("overall_signal").and_then(|v| v.as_str()).unwrap_or("Neutral"));
    let emoji = signal_emoji(&signal);
    let label = signal.to_label();
    let color = signal_color(&signal);

    let recommendation = data.get("recommendation").and_then(|v| v.as_str()).unwrap_or("");
    let mut desc = recommendation.to_string();
    if let Some(tier) = data.get("conviction_tier").and_then(|v| v.as_str()) {
        desc.push_str(&format!("\n**Conviction:** {}", tier));
    }

    let mut embed = CreateEmbed::new()
        .title(format!("{} {} - {}", emoji, symbol, label))
        .description(desc)
        .color(color)
        .image("attachment://chart.png")
        .footer(footer())
        .timestamp(Timestamp::now());

    if let Some(price) = data.get("current_price").and_then(|v| v.as_f64()) {
        embed = embed.field("Price", format!("${:.2}", price), true);
    }
    if let Some(regime) = data.get("market_regime").and_then(|v| v.as_str()) {
        embed = embed.field("Market Regime", regime.replace('_', " "), true);
    }
    if let Some(conf) = data.get("overall_confidence").and_then(|v| v.as_f64()) {
        embed = embed.field("Confidence", format!("{:.0}%", conf * 100.0), true);
    }

    for (key, label_str, icon) in &[
        ("technical", "Technical", "\u{1F4CA}"),
        ("fundamental", "Fundamental", "\u{1F4BC}"),
        ("quantitative", "Quantitative", "\u{1F522}"),
        ("sentiment", "Sentiment", "\u{1F4F0}"),
    ] {
        if let Some(engine) = data.get(*key) {
            let sig = engine.get("signal").and_then(|v| v.as_str()).unwrap_or("?");
            let conf = engine.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let reason = engine.get("reason").and_then(|v| v.as_str()).unwrap_or("");
            let extra = if *key == "quantitative" {
                engine.get("metrics")
                    .and_then(|m| m.get("sharpe_ratio"))
                    .and_then(|v| v.as_f64())
                    .map(|s| format!(" | Sharpe: {:.2}", s))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let text = if reason.is_empty() {
                format!("{} ({:.0}%){}", sig, conf * 100.0, extra)
            } else {
                format!("{} ({:.0}%) - {}{}", sig, conf * 100.0, truncate(reason, 80), extra)
            };
            embed = embed.field(format!("{} {}", icon, label_str), text, false);
        }
    }

    if let Some(horizons) = data.get("time_horizon_signals") {
        let mut horizon_text = String::new();
        for key in &["short_term", "medium_term", "long_term"] {
            if let Some(h) = horizons.get(key) {
                let sig = h.get("signal").and_then(|v| v.as_str()).unwrap_or("N/A");
                let label_t = key.replace('_', " ");
                let label_t = capitalize_first(&label_t);
                horizon_text.push_str(&format!("**{}:** {}\n", label_t, sig));
            }
        }
        if !horizon_text.is_empty() {
            embed = embed.field("\u{1F552} Time Horizons", horizon_text, false);
        }
    }

    if let Some(supp) = data.get("supplementary_signals") {
        let mut supp_fields: Vec<(String, String)> = Vec::new();

        if let Some(sm) = supp.get("smart_money") {
            if let Some(score) = sm.get("score").and_then(|v| v.as_f64()) {
                let sig = sm.get("signal").and_then(|v| v.as_str()).unwrap_or("N/A");
                supp_fields.push(("Smart Money".into(), format!("{:.0} ({})", score, sig)));
            }
        }
        if let Some(ins) = supp.get("insiders") {
            if let Some(sig) = ins.get("signal").and_then(|v| v.as_str()) {
                supp_fields.push(("Insider Activity".into(), sig.to_string()));
            }
        }
        if let Some(opts) = supp.get("options") {
            if let Some(iv) = opts.get("iv_percentile").and_then(|v| v.as_f64()) {
                supp_fields.push(("IV Percentile".into(), format!("{:.0}%", iv * 100.0)));
            }
            if let Some(pcr) = opts.get("put_call_ratio").and_then(|v| v.as_f64()) {
                supp_fields.push(("Put/Call".into(), format!("{:.2}", pcr)));
            }
        }

        for (name, value) in supp_fields {
            embed = embed.field(name, value, true);
        }
    }

    embed
}

pub fn build_price_embed(symbol: &str, snapshot: &SnapshotTicker) -> CreateEmbed {
    let price = snapshot
        .last_trade.as_ref().and_then(|t| t.p)
        .or_else(|| snapshot.day.as_ref().and_then(|d| d.c))
        .unwrap_or(0.0);

    let change = snapshot.todays_change.unwrap_or(0.0);
    let change_pct = snapshot.todays_change_perc.unwrap_or(0.0);
    let color = if change >= 0.0 { COLOR_GREEN } else { COLOR_RED };
    let arrow = if change >= 0.0 { "\u{1F7E2}" } else { "\u{1F534}" };

    let mut embed = CreateEmbed::new()
        .title(format!("{} {} ${:.2}", arrow, symbol, price))
        .color(color)
        .footer(footer())
        .timestamp(Timestamp::now());

    embed = embed.field("Day Change", format!("{:+.2} ({:+.2}%)", change, change_pct), true);

    if let Some(prev) = snapshot.prev_day.as_ref().and_then(|d| d.c) {
        embed = embed.field("Prev Close", format!("${:.2}", prev), true);
    }

    if let Some(day) = &snapshot.day {
        if let (Some(h), Some(l)) = (day.h, day.l) {
            embed = embed.field("Day Range", format!("${:.2} - ${:.2}", l, h), true);
        }
        if let Some(v) = day.v {
            embed = embed.field("Volume", format_volume(v), true);
        }
    }

    embed
}

pub fn build_portfolio_embed(account: &serde_json::Value, positions: &[serde_json::Value]) -> CreateEmbed {
    let equity = parse_f64(account, "equity");
    let buying_power = parse_f64(account, "buying_power");
    let day_pl = parse_f64(account, "equity") - parse_f64(account, "last_equity");
    let color = if day_pl >= 0.0 { COLOR_GREEN } else { COLOR_RED };

    let mut embed = CreateEmbed::new()
        .title("\u{1F4B0} Portfolio Overview")
        .color(color)
        .field("Equity", format!("${:.2}", equity), true)
        .field("Buying Power", format!("${:.2}", buying_power), true)
        .field("Day P&L", format!("{:+.2}", day_pl), true)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if positions.is_empty() {
        embed = embed.description("No open positions");
    } else {
        let mut pos_text = String::new();
        for (i, pos) in positions.iter().take(15).enumerate() {
            let sym = pos.get("symbol").and_then(|v| v.as_str()).unwrap_or("???");
            let qty = parse_f64(pos, "qty");
            let market_val = parse_f64(pos, "market_value");
            let unrealized_pct = parse_f64(pos, "unrealized_plpc") * 100.0;
            let arrow = if unrealized_pct >= 0.0 { "+" } else { "" };
            pos_text.push_str(&format!(
                "**{}** {:.1} shares | ${:.0} | {}{:.1}%\n",
                sym, qty, market_val, arrow, unrealized_pct
            ));
            if i >= 14 {
                pos_text.push_str(&format!("...and {} more", positions.len() - 15));
                break;
            }
        }
        embed = embed.field(format!("Positions ({})", positions.len()), pos_text, false);
    }

    embed
}

pub fn build_backtest_embed(symbol: &str, result: &serde_json::Value) -> CreateEmbed {
    let total_return = result.get("total_return_percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let color = if total_return >= 0.0 { COLOR_GREEN } else { COLOR_RED };

    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F4CA} Backtest: {}", symbol))
        .color(color)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    let fields: &[(&str, &str, &str)] = &[
        ("Total Return", "total_return_percent", "%"),
        ("CAGR", "annualized_return_percent", "%"),
        ("Sharpe Ratio", "sharpe_ratio", ""),
        ("Sortino Ratio", "sortino_ratio", ""),
        ("Max Drawdown", "max_drawdown", "%"),
        ("Win Rate", "win_rate", "%"),
        ("Profit Factor", "profit_factor", ""),
        ("Total Trades", "total_trades", ""),
    ];

    for (label, key, suffix) in fields {
        if let Some(val) = result.get(*key).and_then(|v| v.as_f64()) {
            let formatted = if *suffix == "%" {
                format!("{:.2}%", val)
            } else if key == &"total_trades" {
                format!("{}", val as i64)
            } else {
                format!("{:.2}{}", val, suffix)
            };
            embed = embed.field(*label, formatted, true);
        }
    }

    embed
}

pub fn build_compare_embed_from_json(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let signal = parse_signal(data.get("overall_signal").and_then(|v| v.as_str()).unwrap_or("Neutral"));
    let emoji = signal_emoji(&signal);
    let label = signal.to_label();
    let color = signal_color(&signal);

    let mut embed = CreateEmbed::new()
        .title(format!("{} {}", emoji, symbol))
        .color(color)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    embed = embed.field("Signal", label, true);
    if let Some(conf) = data.get("overall_confidence").and_then(|v| v.as_f64()) {
        embed = embed.field("Confidence", format!("{:.0}%", conf * 100.0), true);
    }
    if let Some(price) = data.get("current_price").and_then(|v| v.as_f64()) {
        embed = embed.field("Price", format!("${:.2}", price), true);
    }

    for key in &["technical", "fundamental", "quantitative", "sentiment"] {
        if let Some(engine) = data.get(*key) {
            let sig = engine.get("signal").and_then(|v| v.as_str()).unwrap_or("?");
            let label_str = capitalize_first(key);
            embed = embed.field(label_str, sig, true);
        }
    }

    let recommendation = data.get("recommendation").and_then(|v| v.as_str()).unwrap_or("");
    embed = embed.description(truncate(recommendation, 200));

    embed
}

pub fn build_help_embed() -> CreateEmbed {
    CreateEmbed::new()
        .title("\u{1F4CA} InvestIQ Bot Commands")
        .color(COLOR_BLUE)
        .description("Use `/iq <command>` to interact with InvestIQ.")
        .field(
            "\u{1F50D} Analysis",
            "`analyze <sym>` - Full 4-engine analysis\n\
             `price <sym>` - Quick price snapshot\n\
             `chart <sym> [days]` - Technical chart\n\
             `compare <sym1> <sym2>` - Side-by-side\n\
             `backtest <sym> [days]` - Backtest results",
            false,
        )
        .field(
            "\u{1F4CB} Portfolio & Watchlist",
            "`portfolio` - Paper trading portfolio\n\
             `watchlist [add|remove|show] [sym]` - Personal watchlist",
            false,
        )
        .field(
            "\u{1F916} Agent Oversight",
            "`agent pending` - Pending trades\n\
             `agent approve <id>` - Approve trade\n\
             `agent reject <id>` - Reject trade\n\
             `agent stats` - Analytics summary\n\
             `agent history` - Recent trades\n\
             `agent regimes` - Win rate by regime",
            false,
        )
        .field(
            "\u{26A0}\u{FE0F} Risk Management",
            "`risk overview` - Portfolio risk radar\n\
             `risk symbol <sym>` - Symbol risk\n\
             `risk breakers` - Circuit breaker status\n\
             `risk positions` - Stop-loss positions",
            false,
        )
        .field(
            "\u{1F4F0} Market Intelligence",
            "`market news <sym>` - Headlines & sentiment\n\
             `market sectors` - Sector rotation\n\
             `market macro` - Macro indicators\n\
             `market screen` - Stock screener",
            false,
        )
        .field(
            "\u{1F4C1} Supplementary Data",
            "`data earnings <sym>` | `data dividends <sym>`\n\
             `data options <sym>` | `data insiders <sym>`\n\
             `data short <sym>` | `data correlation <sym>`",
            false,
        )
        .field(
            "\u{1F4B5} Trading",
            "`trade buy <sym> <shares>` - Buy\n\
             `trade sell <sym> <shares>` - Sell\n\
             `trade close <sym>` - Close position\n\
             `trade orders` - Recent orders",
            false,
        )
        .field(
            "\u{1F4B0} Tax Tools",
            "`tax harvest` - Loss harvesting\n\
             `tax summary [year]` - Year-end summary",
            false,
        )
        .field(
            "\u{1F52C} Advanced Backtesting",
            "`advanced walkforward <sym>` - Walk-forward\n\
             `advanced montecarlo <sym>` - Monte Carlo\n\
             `advanced strategies` - Strategy health",
            false,
        )
        .field(
            "\u{2699}\u{FE0F} System",
            "`system health` - Health check\n\
             `system search <query>` - Symbol search",
            false,
        )
        .footer(footer())
}

// ══════════════════════════════════════════════════════════════════════
// Agent embeds
// ══════════════════════════════════════════════════════════════════════

pub fn build_agent_pending_embed(trades: &[&serde_json::Value]) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F916} Pending Agent Trades ({})", trades.len()))
        .color(COLOR_GOLD)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if trades.is_empty() {
        embed = embed.description("No pending trades.");
    } else {
        let mut text = String::new();
        for (i, trade) in trades.iter().take(15).enumerate() {
            let id = trade.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let sym = trade.get("symbol").and_then(|v| v.as_str()).unwrap_or("???");
            let action = trade.get("action").and_then(|v| v.as_str()).unwrap_or("???");
            let qty = trade.get("quantity").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let conf = trade.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
            text.push_str(&format!(
                "**#{}** {} {} x {:.1} | conf: {:.0}%\n",
                id, action.to_uppercase(), sym, qty, conf * 100.0
            ));
            if i >= 14 {
                text.push_str(&format!("...and {} more", trades.len() - 15));
                break;
            }
        }
        embed = embed.description(text);
    }

    embed
}

pub fn build_agent_stats_embed(data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title("\u{1F4CA} Agent Analytics")
        .color(COLOR_BLUE)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if let Some(total) = data.get("total_trades").and_then(|v| v.as_i64()) {
        embed = embed.field("Total Trades", total.to_string(), true);
    }
    if let Some(wins) = data.get("wins").and_then(|v| v.as_i64()) {
        embed = embed.field("Wins", wins.to_string(), true);
    }
    if let Some(losses) = data.get("losses").and_then(|v| v.as_i64()) {
        embed = embed.field("Losses", losses.to_string(), true);
    }
    if let Some(wr) = data.get("win_rate").and_then(|v| v.as_f64()) {
        embed = embed.field("Win Rate", format!("{:.1}%", wr * 100.0), true);
    }
    if let Some(pnl) = data.get("total_pnl").and_then(|v| v.as_f64()) {
        embed = embed.field("Total P&L", format!("${:+.2}", pnl), true);
    }
    if let Some(gate) = data.get("ml_gate_pass_rate").and_then(|v| v.as_f64()) {
        embed = embed.field("ML Gate Pass", format!("{:.0}%", gate * 100.0), true);
    }

    embed
}

pub fn build_agent_history_embed(trades: &[&serde_json::Value]) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title("\u{1F4DC} Recent Agent Trades")
        .color(COLOR_BLUE)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if trades.is_empty() {
        embed = embed.description("No executed trades.");
    } else {
        let mut text = String::new();
        for trade in trades.iter().take(10) {
            let id = trade.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let sym = trade.get("symbol").and_then(|v| v.as_str()).unwrap_or("???");
            let action = trade.get("action").and_then(|v| v.as_str()).unwrap_or("???");
            let status = trade.get("status").and_then(|v| v.as_str()).unwrap_or("???");
            let pnl = trade.get("pnl").and_then(|v| v.as_f64());
            let pnl_str = pnl.map(|p| format!(" | ${:+.2}", p)).unwrap_or_default();
            text.push_str(&format!(
                "**#{}** {} {} [{}]{}\n",
                id, action.to_uppercase(), sym, status, pnl_str
            ));
        }
        embed = embed.description(text);
    }

    embed
}

pub fn build_agent_regimes_embed(data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title("\u{1F30D} Win Rate by Market Regime")
        .color(COLOR_BLUE)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if let Some(regimes) = data.as_array() {
        for regime in regimes {
            let name = regime.get("regime").and_then(|v| v.as_str()).unwrap_or("Unknown");
            let wr = regime.get("win_rate").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let count = regime.get("trade_count").and_then(|v| v.as_i64()).unwrap_or(0);
            let color_dot = if wr >= 0.6 { "\u{1F7E2}" } else if wr >= 0.4 { "\u{1F7E1}" } else { "\u{1F534}" };
            embed = embed.field(
                format!("{} {}", color_dot, name.replace('_', " ")),
                format!("{:.0}% ({} trades)", wr * 100.0, count),
                true,
            );
        }
        if regimes.is_empty() {
            embed = embed.description("No regime data available yet.");
        }
    } else {
        embed = embed.description("No regime data available yet.");
    }

    embed
}

// ══════════════════════════════════════════════════════════════════════
// Risk embeds
// ══════════════════════════════════════════════════════════════════════

pub fn build_risk_overview_embed(data: &serde_json::Value) -> CreateEmbed {
    let overall = data.get("overall_risk_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let color = if overall >= 70.0 { COLOR_RED } else if overall >= 40.0 { COLOR_GOLD } else { COLOR_GREEN };

    let mut embed = CreateEmbed::new()
        .title(format!("{} Portfolio Risk Radar", risk_indicator(overall)))
        .description(format!("Overall Risk Score: **{:.0}/100**", overall))
        .color(color)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    let dimensions = [
        ("market_risk", "Market Risk"),
        ("volatility_risk", "Volatility"),
        ("concentration_risk", "Concentration"),
        ("liquidity_risk", "Liquidity"),
        ("drawdown_risk", "Drawdown"),
        ("correlation_risk", "Correlation"),
    ];

    for (key, label) in &dimensions {
        if let Some(score) = data.get(*key).and_then(|v| v.as_f64()) {
            embed = embed.field(
                format!("{} {}", risk_indicator(score), label),
                format!("{:.0}/100", score),
                true,
            );
        }
    }

    embed
}

pub fn build_risk_symbol_embed(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let overall = data.get("overall_risk_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let color = if overall >= 70.0 { COLOR_RED } else if overall >= 40.0 { COLOR_GOLD } else { COLOR_GREEN };

    let mut embed = CreateEmbed::new()
        .title(format!("{} {} Risk Analysis", risk_indicator(overall), symbol))
        .description(format!("Risk Score: **{:.0}/100**", overall))
        .color(color)
        .footer(footer())
        .timestamp(Timestamp::now());

    let dimensions = [
        ("market_risk", "Market Risk"),
        ("volatility_risk", "Volatility"),
        ("concentration_risk", "Concentration"),
        ("liquidity_risk", "Liquidity"),
        ("drawdown_risk", "Drawdown"),
        ("correlation_risk", "Correlation"),
    ];

    for (key, label) in &dimensions {
        if let Some(score) = data.get(*key).and_then(|v| v.as_f64()) {
            embed = embed.field(
                format!("{} {}", risk_indicator(score), label),
                format!("{:.0}/100", score),
                true,
            );
        }
    }

    embed
}

pub fn build_circuit_breakers_embed(data: &serde_json::Value) -> CreateEmbed {
    let halted = data.get("trading_halted").and_then(|v| v.as_bool()).unwrap_or(false);
    let color = if halted { COLOR_RED } else { COLOR_GREEN };
    let status = if halted { "\u{1F6D1} HALTED" } else { "\u{1F7E2} Active" };

    let mut embed = CreateEmbed::new()
        .title(format!("Circuit Breakers — {}", status))
        .color(color)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if halted {
        if let Some(reason) = data.get("halt_reason").and_then(|v| v.as_str()) {
            embed = embed.description(format!("**Reason:** {}", reason));
        }
    }

    if let Some(v) = data.get("daily_loss_limit_percent").and_then(|v| v.as_f64()) {
        embed = embed.field("Daily Loss Limit", format!("{:.1}%", v), true);
    }
    if let Some(v) = data.get("max_consecutive_losses").and_then(|v| v.as_i64()) {
        embed = embed.field("Max Consec. Losses", v.to_string(), true);
    }
    if let Some(v) = data.get("account_drawdown_limit_percent").and_then(|v| v.as_f64()) {
        embed = embed.field("Drawdown Limit", format!("{:.1}%", v), true);
    }
    if let Some(v) = data.get("consecutive_losses").and_then(|v| v.as_i64()) {
        embed = embed.field("Current Consec. Losses", v.to_string(), true);
    }

    embed
}

pub fn build_risk_positions_embed(positions: &[serde_json::Value]) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F6E1}\u{FE0F} Tracked Positions ({})", positions.len()))
        .color(COLOR_BLUE)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if positions.is_empty() {
        embed = embed.description("No stop-loss tracked positions.");
    } else {
        let mut text = String::new();
        for (i, pos) in positions.iter().take(15).enumerate() {
            let sym = pos.get("symbol").and_then(|v| v.as_str()).unwrap_or("???");
            let sl = pos.get("stop_loss").and_then(|v| v.as_f64());
            let tp = pos.get("take_profit").and_then(|v| v.as_f64());
            let trailing = pos.get("trailing_stop").and_then(|v| v.as_bool()).unwrap_or(false);

            let mut info = format!("**{}**", sym);
            if let Some(sl) = sl {
                info.push_str(&format!(" | SL: ${:.2}", sl));
            }
            if let Some(tp) = tp {
                info.push_str(&format!(" | TP: ${:.2}", tp));
            }
            if trailing {
                info.push_str(" | Trailing");
            }
            text.push_str(&info);
            text.push('\n');
            if i >= 14 {
                text.push_str(&format!("...and {} more", positions.len() - 15));
                break;
            }
        }
        embed = embed.description(text);
    }

    embed
}

// ══════════════════════════════════════════════════════════════════════
// Market embeds
// ══════════════════════════════════════════════════════════════════════

pub fn build_news_embed(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let sentiment = data.get("overall_sentiment").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let color = if sentiment > 0.1 { COLOR_GREEN } else if sentiment < -0.1 { COLOR_RED } else { COLOR_GOLD };

    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F4F0} {} News & Sentiment", symbol))
        .color(color)
        .footer(footer())
        .timestamp(Timestamp::now());

    embed = embed.field("Sentiment Score", format!("{:+.2}", sentiment), true);

    if let Some(pos) = data.get("positive_percent").and_then(|v| v.as_f64()) {
        embed = embed.field("Positive", format!("{:.0}%", pos), true);
    }
    if let Some(neg) = data.get("negative_percent").and_then(|v| v.as_f64()) {
        embed = embed.field("Negative", format!("{:.0}%", neg), true);
    }
    if let Some(buzz) = data.get("buzz_score").and_then(|v| v.as_f64()) {
        embed = embed.field("Buzz", format!("{:.1}x", buzz), true);
    }

    if let Some(headlines) = data.get("headlines").and_then(|v| v.as_array()) {
        let mut news_text = String::new();
        for hl in headlines.iter().take(5) {
            let title = hl.get("title").and_then(|v| v.as_str()).unwrap_or("No title");
            let sent = hl.get("sentiment").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let icon = if sent > 0.1 { "\u{1F7E2}" } else if sent < -0.1 { "\u{1F534}" } else { "\u{26AA}" };
            news_text.push_str(&format!("{} {}\n", icon, truncate(title, 80)));
        }
        if !news_text.is_empty() {
            embed = embed.field("Headlines", news_text, false);
        }
    }

    embed
}

pub fn build_sectors_embed(data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title("\u{1F4CA} Sector Rotation")
        .color(COLOR_BLUE)
        .footer(footer())
        .timestamp(Timestamp::now());

    if let Some(summary) = data.get("market_summary") {
        if let Some(regime) = summary.get("regime").and_then(|v| v.as_str()) {
            embed = embed.description(format!("Market Regime: **{}**", regime.replace('_', " ")));
        }
    }

    if let Some(sectors) = data.get("sectors").and_then(|v| v.as_array()) {
        for sector in sectors.iter().take(11) {
            let name = sector.get("name").and_then(|v| v.as_str())
                .or_else(|| sector.get("symbol").and_then(|v| v.as_str()))
                .unwrap_or("???");
            let ret = sector.get("return_1d").and_then(|v| v.as_f64())
                .or_else(|| sector.get("change_percent").and_then(|v| v.as_f64()))
                .unwrap_or(0.0);
            let icon = if ret > 0.0 { "\u{1F7E2}" } else if ret < 0.0 { "\u{1F534}" } else { "\u{26AA}" };
            embed = embed.field(
                format!("{} {}", icon, name),
                format!("{:+.2}%", ret),
                true,
            );
        }
    }

    embed
}

pub fn build_macro_embed(data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title("\u{1F30D} Macro Indicators")
        .color(COLOR_BLUE)
        .footer(footer())
        .timestamp(Timestamp::now());

    if let Some(regime) = data.get("market_regime").and_then(|v| v.as_str()) {
        embed = embed.description(format!("Market Regime: **{}**", regime.replace('_', " ")));
    }

    let fields = [
        ("spy_trend", "SPY Trend"),
        ("rate_environment", "Rate Environment"),
        ("volatility_level", "Volatility"),
        ("risk_appetite", "Risk Appetite"),
        ("dollar_trend", "Dollar Trend"),
    ];

    for (key, label) in &fields {
        if let Some(val) = data.get(*key).and_then(|v| v.as_str()) {
            embed = embed.field(*label, val.replace('_', " "), true);
        } else if let Some(val) = data.get(*key).and_then(|v| v.as_f64()) {
            embed = embed.field(*label, format!("{:.2}", val), true);
        }
    }

    embed
}

pub fn build_screen_embed(data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title("\u{1F50E} Stock Screener")
        .color(COLOR_BLUE)
        .footer(footer())
        .timestamp(Timestamp::now());

    if let Some(opportunities) = data.get("opportunities").and_then(|v| v.as_array())
        .or_else(|| data.as_array())
    {
        if opportunities.is_empty() {
            embed = embed.description("No opportunities found.");
        } else {
            let mut text = String::new();
            for opp in opportunities.iter().take(10) {
                let sym = opp.get("symbol").and_then(|v| v.as_str()).unwrap_or("???");
                let signal = opp.get("signal").and_then(|v| v.as_str()).unwrap_or("N/A");
                let score = opp.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                text.push_str(&format!("**{}** — {} ({:.0})\n", sym, signal, score));
            }
            embed = embed.description(text);
        }
    } else {
        embed = embed.description("No screener data.");
    }

    embed
}

// ══════════════════════════════════════════════════════════════════════
// Data embeds
// ══════════════════════════════════════════════════════════════════════

pub fn build_earnings_embed(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F4C8} {} Earnings", symbol))
        .color(COLOR_BLUE)
        .footer(footer())
        .timestamp(Timestamp::now());

    if let Some(eps) = data.get("eps_actual").and_then(|v| v.as_f64()) {
        embed = embed.field("EPS (Actual)", format!("${:.2}", eps), true);
    }
    if let Some(est) = data.get("eps_estimate").and_then(|v| v.as_f64()) {
        embed = embed.field("EPS (Est.)", format!("${:.2}", est), true);
    }
    if let Some(surprise) = data.get("eps_surprise_percent").and_then(|v| v.as_f64()) {
        embed = embed.field("Surprise", format!("{:+.1}%", surprise), true);
    }
    if let Some(rev) = data.get("revenue").and_then(|v| v.as_f64()) {
        embed = embed.field("Revenue", format!("${}", format_volume(rev)), true);
    }
    if let Some(growth) = data.get("revenue_growth").and_then(|v| v.as_f64()) {
        embed = embed.field("Rev Growth", format!("{:+.1}%", growth * 100.0), true);
    }
    if let Some(date) = data.get("next_earnings_date").and_then(|v| v.as_str()) {
        embed = embed.field("Next Earnings", date, true);
    }
    if let Some(source) = data.get("data_source").and_then(|v| v.as_str()) {
        embed = embed.field("Source", source, true);
    }

    embed
}

pub fn build_dividends_embed(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F4B5} {} Dividends", symbol))
        .color(COLOR_GREEN)
        .footer(footer())
        .timestamp(Timestamp::now());

    if let Some(yld) = data.get("yield_percent").and_then(|v| v.as_f64()) {
        embed = embed.field("Yield", format!("{:.2}%", yld), true);
    }
    if let Some(amt) = data.get("amount").and_then(|v| v.as_f64()) {
        embed = embed.field("Amount", format!("${:.4}", amt), true);
    }
    if let Some(freq) = data.get("frequency").and_then(|v| v.as_str()) {
        embed = embed.field("Frequency", freq, true);
    }
    if let Some(ex) = data.get("ex_date").and_then(|v| v.as_str()) {
        embed = embed.field("Ex-Date", ex, true);
    }
    if let Some(pay) = data.get("pay_date").and_then(|v| v.as_str()) {
        embed = embed.field("Pay Date", pay, true);
    }
    if let Some(source) = data.get("data_source").and_then(|v| v.as_str()) {
        embed = embed.field("Source", source, true);
    }

    embed
}

pub fn build_options_embed(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F4CA} {} Options Flow", symbol))
        .color(COLOR_BLUE)
        .footer(footer())
        .timestamp(Timestamp::now());

    if let Some(iv) = data.get("iv_percentile").and_then(|v| v.as_f64()) {
        embed = embed.field("IV Percentile", format!("{:.0}%", iv * 100.0), true);
    }
    if let Some(pcr) = data.get("put_call_ratio").and_then(|v| v.as_f64()) {
        embed = embed.field("Put/Call Ratio", format!("{:.2}", pcr), true);
    }
    if let Some(sig) = data.get("put_call_signal").and_then(|v| v.as_str()) {
        embed = embed.field("Signal", sig, true);
    }
    if let Some(skew) = data.get("iv_skew").and_then(|v| v.as_f64()) {
        embed = embed.field("IV Skew", format!("{:.2}", skew), true);
    }
    if let Some(mp) = data.get("max_pain").and_then(|v| v.as_f64()) {
        embed = embed.field("Max Pain", format!("${:.2}", mp), true);
    }
    if let Some(source) = data.get("data_source").and_then(|v| v.as_str()) {
        embed = embed.field("Source", source, true);
    }

    embed
}

pub fn build_insiders_embed(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F575}\u{FE0F} {} Insider Activity", symbol))
        .color(COLOR_BLUE)
        .footer(footer())
        .timestamp(Timestamp::now());

    if let Some(sig) = data.get("signal").and_then(|v| v.as_str()) {
        embed = embed.field("Signal", sig, true);
    }
    if let Some(net) = data.get("net_value").and_then(|v| v.as_f64()) {
        let sign = if net >= 0.0 { "Net Buy" } else { "Net Sell" };
        embed = embed.field(sign, format!("${}", format_volume(net.abs())), true);
    }
    if let Some(exec_buys) = data.get("executive_buy_count").and_then(|v| v.as_i64()) {
        embed = embed.field("Exec Buys", exec_buys.to_string(), true);
    }

    if let Some(transactions) = data.get("transactions").and_then(|v| v.as_array()) {
        let mut text = String::new();
        for tx in transactions.iter().take(5) {
            let name = tx.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
            let title = tx.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let tx_type = tx.get("type").and_then(|v| v.as_str()).unwrap_or("???");
            let shares = tx.get("shares").and_then(|v| v.as_f64()).unwrap_or(0.0);
            text.push_str(&format!("{} ({}) — {} {:.0}\n", name, title, tx_type, shares));
        }
        if !text.is_empty() {
            embed = embed.field("Recent Transactions", text, false);
        }
    }

    if let Some(source) = data.get("data_source").and_then(|v| v.as_str()) {
        embed = embed.field("Source", source, true);
    }

    embed
}

pub fn build_short_interest_embed(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let score = data.get("squeeze_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let color = if score >= 70.0 { COLOR_RED } else if score >= 40.0 { COLOR_GOLD } else { COLOR_GREEN };

    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F4C9} {} Short Interest", symbol))
        .description(format!("Squeeze Score: **{:.0}/100**", score))
        .color(color)
        .footer(footer())
        .timestamp(Timestamp::now());

    let components = [
        ("volume_spike", "Volume Spike"),
        ("volume_trend", "Volume Trend"),
        ("momentum", "Momentum"),
        ("volatility", "Volatility"),
        ("bb_squeeze", "BB Squeeze"),
        ("rsi_score", "RSI"),
    ];

    for (key, label) in &components {
        if let Some(v) = data.get(*key).and_then(|v| v.as_f64()) {
            embed = embed.field(*label, format!("{:.0}", v), true);
        }
    }

    if let Some(interp) = data.get("interpretation").and_then(|v| v.as_str()) {
        embed = embed.field("Interpretation", truncate(interp, 200), false);
    }
    if let Some(source) = data.get("data_source").and_then(|v| v.as_str()) {
        embed = embed.field("Source", source, true);
    }

    embed
}

pub fn build_correlation_embed(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F517} {} Correlations", symbol))
        .color(COLOR_BLUE)
        .footer(footer())
        .timestamp(Timestamp::now());

    let benchmarks = ["SPY", "QQQ", "DIA", "IWM"];
    if let Some(corrs) = data.get("correlations").and_then(|v| v.as_object()) {
        for bm in &benchmarks {
            if let Some(val) = corrs.get(*bm).and_then(|v| v.as_f64()) {
                let bar = if val.abs() >= 0.7 { "\u{1F7E0}" } else if val.abs() >= 0.4 { "\u{1F7E1}" } else { "\u{1F7E2}" };
                embed = embed.field(
                    format!("{} vs {}", bar, bm),
                    format!("{:.3}", val),
                    true,
                );
            }
        }
    }

    if let Some(beta) = data.get("beta").and_then(|v| v.as_f64()) {
        embed = embed.field("Beta (SPY)", format!("{:.2}", beta), true);
    }
    if let Some(rolling) = data.get("rolling_30d_spy").and_then(|v| v.as_f64()) {
        embed = embed.field("30d Rolling (SPY)", format!("{:.3}", rolling), true);
    }
    if let Some(source) = data.get("data_source").and_then(|v| v.as_str()) {
        embed = embed.field("Source", source, true);
    }

    embed
}

// ══════════════════════════════════════════════════════════════════════
// Trading embeds
// ══════════════════════════════════════════════════════════════════════

pub fn build_orders_embed(orders: &[serde_json::Value]) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F4CB} Recent Orders ({})", orders.len()))
        .color(COLOR_BLUE)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if orders.is_empty() {
        embed = embed.description("No recent orders.");
    } else {
        let mut text = String::new();
        for order in orders.iter().take(10) {
            let sym = order.get("symbol").and_then(|v| v.as_str()).unwrap_or("???");
            let side = order.get("side").and_then(|v| v.as_str()).unwrap_or("???");
            let qty = parse_f64(order, "qty");
            let status = order.get("status").and_then(|v| v.as_str()).unwrap_or("???");
            let icon = match status {
                "filled" => "\u{2705}",
                "cancelled" | "canceled" => "\u{274C}",
                "new" | "accepted" => "\u{23F3}",
                _ => "\u{2B1C}",
            };
            text.push_str(&format!(
                "{} **{}** {} x {:.1} [{}]\n",
                icon, side.to_uppercase(), sym, qty, status
            ));
        }
        if orders.len() > 10 {
            text.push_str(&format!("...and {} more", orders.len() - 10));
        }
        embed = embed.description(text);
    }

    embed
}

// ══════════════════════════════════════════════════════════════════════
// Tax embeds
// ══════════════════════════════════════════════════════════════════════

pub fn build_tax_harvest_embed(data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title("\u{1F33E} Tax-Loss Harvesting Opportunities")
        .color(COLOR_GOLD)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if let Some(opps) = data.get("opportunities").and_then(|v| v.as_array())
        .or_else(|| data.as_array())
    {
        if opps.is_empty() {
            embed = embed.description("No harvest opportunities found.");
        } else {
            let mut text = String::new();
            for opp in opps.iter().take(10) {
                let sym = opp.get("symbol").and_then(|v| v.as_str()).unwrap_or("???");
                let loss = opp.get("unrealized_loss").and_then(|v| v.as_f64()).unwrap_or(0.0);
                text.push_str(&format!("**{}** — ${:.2} harvestable loss\n", sym, loss.abs()));
            }
            embed = embed.description(text);
        }
    } else {
        embed = embed.description("No harvest data available.");
    }

    embed
}

pub fn build_tax_summary_embed(year: i64, data: &serde_json::Value) -> CreateEmbed {
    let net = data.get("net_gain_loss").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let color = pnl_color(net);

    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F4B0} {} Tax Summary", year))
        .color(color)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if let Some(st_gains) = data.get("short_term_gains").and_then(|v| v.as_f64()) {
        embed = embed.field("ST Gains", format!("${:.2}", st_gains), true);
    }
    if let Some(st_losses) = data.get("short_term_losses").and_then(|v| v.as_f64()) {
        embed = embed.field("ST Losses", format!("${:.2}", st_losses), true);
    }
    if let Some(lt_gains) = data.get("long_term_gains").and_then(|v| v.as_f64()) {
        embed = embed.field("LT Gains", format!("${:.2}", lt_gains), true);
    }
    if let Some(lt_losses) = data.get("long_term_losses").and_then(|v| v.as_f64()) {
        embed = embed.field("LT Losses", format!("${:.2}", lt_losses), true);
    }
    embed = embed.field("Net Gain/Loss", format!("${:+.2}", net), true);

    embed
}

// ══════════════════════════════════════════════════════════════════════
// Advanced embeds
// ══════════════════════════════════════════════════════════════════════

pub fn build_walkforward_embed(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F504} Walk-Forward: {}", symbol))
        .color(COLOR_BLUE)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if let Some(is_sharpe) = data.get("in_sample_sharpe").and_then(|v| v.as_f64()) {
        embed = embed.field("IS Sharpe", format!("{:.2}", is_sharpe), true);
    }
    if let Some(oos_sharpe) = data.get("out_of_sample_sharpe").and_then(|v| v.as_f64()) {
        embed = embed.field("OOS Sharpe", format!("{:.2}", oos_sharpe), true);
    }
    if let Some(ratio) = data.get("overfitting_ratio").and_then(|v| v.as_f64()) {
        let icon = if ratio > 0.5 { "\u{1F534}" } else { "\u{1F7E2}" };
        embed = embed.field("Overfit Ratio", format!("{} {:.2}", icon, ratio), true);
    }
    if let Some(folds) = data.get("num_folds").and_then(|v| v.as_i64()) {
        embed = embed.field("Folds", folds.to_string(), true);
    }
    if let Some(ret) = data.get("oos_total_return_percent").and_then(|v| v.as_f64()) {
        embed = embed.field("OOS Return", format!("{:.2}%", ret), true);
    }

    embed
}

pub fn build_montecarlo_embed(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F3B2} Monte Carlo: {}", symbol))
        .color(COLOR_BLUE)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if let Some(median) = data.get("median_return_percent").and_then(|v| v.as_f64()) {
        embed = embed.field("Median Return", format!("{:.2}%", median), true);
    }
    if let Some(p5) = data.get("percentile_5").and_then(|v| v.as_f64()) {
        embed = embed.field("5th Percentile", format!("{:.2}%", p5), true);
    }
    if let Some(p95) = data.get("percentile_95").and_then(|v| v.as_f64()) {
        embed = embed.field("95th Percentile", format!("{:.2}%", p95), true);
    }
    if let Some(prob) = data.get("probability_of_profit").and_then(|v| v.as_f64()) {
        embed = embed.field("Prob. of Profit", format!("{:.1}%", prob * 100.0), true);
    }
    if let Some(mean) = data.get("mean_return_percent").and_then(|v| v.as_f64()) {
        embed = embed.field("Mean Return", format!("{:.2}%", mean), true);
    }
    if let Some(sims) = data.get("num_simulations").and_then(|v| v.as_i64()) {
        embed = embed.field("Simulations", sims.to_string(), true);
    }

    embed
}

pub fn build_strategies_embed(data: &serde_json::Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title("\u{1F4C9} Strategy Health")
        .color(COLOR_BLUE)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if let Some(strategies) = data.get("strategies").and_then(|v| v.as_array())
        .or_else(|| data.as_array())
    {
        if strategies.is_empty() {
            embed = embed.description("No strategies tracked. Run a backtest first.");
        } else {
            let mut text = String::new();
            for strat in strategies.iter().take(10) {
                let name = strat.get("name").and_then(|v| v.as_str()).unwrap_or("???");
                let health = strat.get("health_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let decay = strat.get("decay_percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let icon = if health >= 70.0 { "\u{1F7E2}" } else if health >= 40.0 { "\u{1F7E1}" } else { "\u{1F534}" };
                text.push_str(&format!(
                    "{} **{}** — Health: {:.0} | Decay: {:.1}%\n",
                    icon, name, health, decay
                ));
            }
            embed = embed.description(text);
        }
    } else {
        embed = embed.description("No strategy data available.");
    }

    embed
}

// ══════════════════════════════════════════════════════════════════════
// System embeds
// ══════════════════════════════════════════════════════════════════════

pub fn build_health_embed(status_code: u16, data: &serde_json::Value) -> CreateEmbed {
    let overall = if status_code == 200 { "\u{1F7E2} Healthy" }
        else if status_code == 503 { "\u{1F534} Degraded" }
        else { "\u{1F7E1} Unknown" };
    let color = if status_code == 200 { COLOR_GREEN } else { COLOR_RED };

    let mut embed = CreateEmbed::new()
        .title(format!("{} System Health", overall))
        .color(color)
        .footer(footer_plain())
        .timestamp(Timestamp::now());

    if let Some(checks) = data.get("checks").and_then(|v| v.as_object()) {
        for (name, check) in checks {
            let status = check.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
            let latency = check.get("latency_ms").and_then(|v| v.as_i64()).unwrap_or(0);
            let error = check.get("error").and_then(|v| v.as_str());
            let icon = match status {
                "ok" | "healthy" => "\u{2705}",
                "error" | "unhealthy" => "\u{274C}",
                "degraded" => "\u{26A0}\u{FE0F}",
                _ => "\u{2B1C}",
            };
            let detail = if let Some(err) = error {
                format!("{} {}ms — {}", icon, latency, truncate(err, 60))
            } else {
                format!("{} {}ms", icon, latency)
            };
            embed = embed.field(capitalize_first(name), detail, true);
        }
    }

    embed
}

pub fn build_search_embed(query: &str, results: &[serde_json::Value]) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F50D} Search: \"{}\"", truncate(query, 30)))
        .color(COLOR_BLUE)
        .footer(footer())
        .timestamp(Timestamp::now());

    if results.is_empty() {
        embed = embed.description("No matching symbols found.");
    } else {
        let mut text = String::new();
        for result in results.iter().take(10) {
            let ticker = result.get("ticker").and_then(|v| v.as_str()).unwrap_or("???");
            let name = result.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let market = result.get("market").and_then(|v| v.as_str()).unwrap_or("");
            text.push_str(&format!("**{}** — {} ({})\n", ticker, truncate(name, 40), market));
        }
        embed = embed.description(text);
    }

    embed
}
