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
        SignalStrength::StrongBuy => "\u{1F680}",  // rocket
        SignalStrength::Buy => "\u{1F4C8}",        // chart_up
        SignalStrength::WeakBuy => "\u{2197}\u{FE0F}",
        SignalStrength::Neutral => "\u{27A1}\u{FE0F}",
        SignalStrength::WeakSell => "\u{2198}\u{FE0F}",
        SignalStrength::Sell => "\u{1F4C9}",       // chart_down
        SignalStrength::StrongSell => "\u{26A0}\u{FE0F}",
    }
}

/// Build analysis embed from API JSON response (uses all server-side enhancements)
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
        .footer(serenity::builder::CreateEmbedFooter::new(
            "InvestIQ | Powered by Polygon.io",
        ))
        .timestamp(Timestamp::now());

    // Price + regime
    if let Some(price) = data.get("current_price").and_then(|v| v.as_f64()) {
        embed = embed.field("Price", format!("${:.2}", price), true);
    }
    if let Some(regime) = data.get("market_regime").and_then(|v| v.as_str()) {
        embed = embed.field("Market Regime", regime.replace('_', " "), true);
    }
    if let Some(conf) = data.get("overall_confidence").and_then(|v| v.as_f64()) {
        embed = embed.field("Confidence", format!("{:.0}%", conf * 100.0), true);
    }

    // Engine results
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

    // Time horizon signals
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

    // Supplementary signals
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

pub fn build_price_embed(symbol: &str, snapshot: &SnapshotTicker) -> CreateEmbed {
    let price = snapshot
        .last_trade
        .as_ref()
        .and_then(|t| t.p)
        .or_else(|| snapshot.day.as_ref().and_then(|d| d.c))
        .unwrap_or(0.0);

    let change = snapshot.todays_change.unwrap_or(0.0);
    let change_pct = snapshot.todays_change_perc.unwrap_or(0.0);
    let color = if change >= 0.0 { COLOR_GREEN } else { COLOR_RED };
    let arrow = if change >= 0.0 { "\u{1F7E2}" } else { "\u{1F534}" };

    let mut embed = CreateEmbed::new()
        .title(format!("{} {} ${:.2}", arrow, symbol, price))
        .color(color)
        .footer(serenity::builder::CreateEmbedFooter::new(
            "InvestIQ | Powered by Polygon.io",
        ))
        .timestamp(Timestamp::now());

    embed = embed.field(
        "Day Change",
        format!("{:+.2} ({:+.2}%)", change, change_pct),
        true,
    );

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

pub fn build_portfolio_embed(
    account: &serde_json::Value,
    positions: &[serde_json::Value],
) -> CreateEmbed {
    let equity = parse_f64(account, "equity");
    let buying_power = parse_f64(account, "buying_power");
    let day_pl = parse_f64(account, "equity")
        - parse_f64(account, "last_equity");

    let color = if day_pl >= 0.0 { COLOR_GREEN } else { COLOR_RED };

    let mut embed = CreateEmbed::new()
        .title("\u{1F4B0} Portfolio Overview")
        .color(color)
        .field("Equity", format!("${:.2}", equity), true)
        .field("Buying Power", format!("${:.2}", buying_power), true)
        .field(
            "Day P&L",
            format!("{:+.2}", day_pl),
            true,
        )
        .footer(serenity::builder::CreateEmbedFooter::new("InvestIQ"))
        .timestamp(Timestamp::now());

    if positions.is_empty() {
        embed = embed.description("No open positions");
    } else {
        let mut pos_text = String::new();
        for (i, pos) in positions.iter().take(15).enumerate() {
            let sym = pos
                .get("symbol")
                .and_then(|v| v.as_str())
                .unwrap_or("???");
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
        embed = embed.field(
            format!("Positions ({})", positions.len()),
            pos_text,
            false,
        );
    }

    embed
}

pub fn build_backtest_embed(symbol: &str, result: &serde_json::Value) -> CreateEmbed {
    let total_return = result
        .get("total_return_percent")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let color = if total_return >= 0.0 {
        COLOR_GREEN
    } else {
        COLOR_RED
    };

    let mut embed = CreateEmbed::new()
        .title(format!("\u{1F4CA} Backtest: {}", symbol))
        .color(color)
        .footer(serenity::builder::CreateEmbedFooter::new("InvestIQ"))
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

/// Build compare embed from API JSON response
pub fn build_compare_embed_from_json(symbol: &str, data: &serde_json::Value) -> CreateEmbed {
    let signal = parse_signal(data.get("overall_signal").and_then(|v| v.as_str()).unwrap_or("Neutral"));
    let emoji = signal_emoji(&signal);
    let label = signal.to_label();
    let color = signal_color(&signal);

    let mut embed = CreateEmbed::new()
        .title(format!("{} {}", emoji, symbol))
        .color(color)
        .footer(serenity::builder::CreateEmbedFooter::new("InvestIQ"))
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
        .field(
            "/iq analyze <symbol>",
            "Full 4-engine analysis with chart",
            false,
        )
        .field(
            "/iq price <symbol>",
            "Quick price snapshot",
            false,
        )
        .field(
            "/iq chart <symbol> [days]",
            "Enhanced technical chart (default 90 days)",
            false,
        )
        .field(
            "/iq watchlist [add|remove|show] [symbol]",
            "Manage your personal watchlist",
            false,
        )
        .field(
            "/iq portfolio",
            "View paper trading portfolio",
            false,
        )
        .field(
            "/iq compare <symbol1> <symbol2>",
            "Side-by-side analysis comparison",
            false,
        )
        .field(
            "/iq backtest <symbol> [days]",
            "Backtest results summary (default 365 days)",
            false,
        )
        .field("/iq help", "Show this help message", false)
        .footer(serenity::builder::CreateEmbedFooter::new(
            "InvestIQ | Powered by Polygon.io",
        ))
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

fn format_volume(v: f64) -> String {
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

fn parse_f64(val: &serde_json::Value, key: &str) -> f64 {
    val.get(key)
        .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(0.0)
}
