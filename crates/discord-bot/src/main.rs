mod charts;
mod embeds;

use analysis_core::SignalStrength;
use serenity::{
    all::{
        Command, CommandDataOption, CommandDataOptionValue, CommandInteraction, CommandOptionType,
        CreateCommand, CreateCommandOption, CreateInteractionResponse,
        CreateInteractionResponseMessage, EditInteractionResponse, Interaction,
    },
    async_trait,
    builder::{CreateAttachment, CreateEmbed, CreateMessage},
    model::{channel::Message, gateway::Ready, id::UserId},
    prelude::*,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Instant;
use tokio::signal::unix::SignalKind;
use tokio::sync::RwLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const API_BASE_URL: &str = "http://localhost:3000";
const RATE_LIMIT_COMMANDS: u32 = 5;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;
const MAX_WATCHLIST_SIZE: usize = 20;

/// Simple circuit breaker for the API server. After `THRESHOLD` consecutive
/// failures, skip API calls for `COOLDOWN_SECS` seconds to let it recover.
struct CircuitBreaker {
    consecutive_failures: AtomicU32,
    open_until_epoch_secs: AtomicU64,
}

const CIRCUIT_BREAKER_THRESHOLD: u32 = 5;
const CIRCUIT_BREAKER_COOLDOWN_SECS: u64 = 30;

impl CircuitBreaker {
    fn new() -> Self {
        Self {
            consecutive_failures: AtomicU32::new(0),
            open_until_epoch_secs: AtomicU64::new(0),
        }
    }

    /// Returns true if the circuit is open (API calls should be skipped).
    fn is_open(&self) -> bool {
        let until = self.open_until_epoch_secs.load(Ordering::Relaxed);
        if until == 0 {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now >= until {
            // Cooldown expired — half-open, reset
            self.open_until_epoch_secs.store(0, Ordering::Relaxed);
            self.consecutive_failures.store(0, Ordering::Relaxed);
            false
        } else {
            true
        }
    }

    fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.open_until_epoch_secs.store(0, Ordering::Relaxed);
    }

    fn record_failure(&self) {
        let count = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= CIRCUIT_BREAKER_THRESHOLD {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.open_until_epoch_secs
                .store(now + CIRCUIT_BREAKER_COOLDOWN_SECS, Ordering::Relaxed);
            tracing::warn!(
                "Circuit breaker OPEN: {} consecutive API failures, skipping for {}s",
                count,
                CIRCUIT_BREAKER_COOLDOWN_SECS
            );
        }
    }
}

struct Handler {
    http_client: reqwest::Client,
    api_base: String,
    polygon_client: Arc<polygon_client::PolygonClient>,
    watchlists: Arc<RwLock<HashMap<UserId, Vec<String>>>>,
    rate_limits: Arc<RwLock<HashMap<UserId, (Instant, u32)>>>,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl Handler {
    async fn check_rate_limit(&self, user_id: UserId) -> Result<(), u64> {
        let mut limits = self.rate_limits.write().await;
        let now = Instant::now();

        if limits.len() > 1000 {
            limits.retain(|_, (ts, _)| now.duration_since(*ts).as_secs() < RATE_LIMIT_WINDOW_SECS);
        }

        if let Some((window_start, count)) = limits.get_mut(&user_id) {
            let elapsed = now.duration_since(*window_start).as_secs();
            if elapsed >= RATE_LIMIT_WINDOW_SECS {
                *window_start = now;
                *count = 1;
                Ok(())
            } else if *count >= RATE_LIMIT_COMMANDS {
                Err(RATE_LIMIT_WINDOW_SECS - elapsed)
            } else {
                *count += 1;
                Ok(())
            }
        } else {
            limits.insert(user_id, (now, 1));
            Ok(())
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };

        let user_id = command.user.id;

        if let Err(wait_secs) = self.check_rate_limit(user_id).await {
            let msg = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(format!("Rate limited. Try again in {}s.", wait_secs))
                    .ephemeral(true),
            );
            let _ = command.create_response(&ctx.http, msg).await;
            return;
        }

        if command.data.name != "iq" {
            return;
        }

        let subcommand = match command.data.options.first() {
            Some(opt) => opt,
            None => {
                let _ = respond_ephemeral(&ctx, &command, "Use `/iq help` for commands.").await;
                return;
            }
        };

        match subcommand.name.as_str() {
            "analyze" => self.handle_analyze(&ctx, &command, subcommand).await,
            "price" => self.handle_price(&ctx, &command, subcommand).await,
            "chart" => self.handle_chart(&ctx, &command, subcommand).await,
            "watchlist" => self.handle_watchlist(&ctx, &command, subcommand).await,
            "portfolio" => self.handle_portfolio(&ctx, &command).await,
            "compare" => self.handle_compare(&ctx, &command, subcommand).await,
            "backtest" => self.handle_backtest(&ctx, &command, subcommand).await,
            "help" => self.handle_help(&ctx, &command).await,
            _ => {
                let _ =
                    respond_ephemeral(&ctx, &command, "Unknown subcommand. Use `/iq help`.").await;
            }
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot || !msg.content.starts_with("!iq") {
            return;
        }

        let _ = msg
            .channel_id
            .say(
                &ctx.http,
                "**Note:** `!iq` prefix commands are deprecated. Please use `/iq` slash commands instead.\nUse `/iq help` for the full command list.",
            )
            .await;

        let parts: Vec<&str> = msg.content.split_whitespace().collect();
        if parts.len() >= 3 && parts[1] == "analyze" {
            let symbol = parts[2].to_uppercase();
            let _ = msg.channel_id.broadcast_typing(&ctx.http).await;

            let url = format!("{}/api/analyze/{}", self.api_base, symbol);
            match self.api_get(&url).await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<serde_json::Value>().await {
                        Ok(json) => {
                            let data = json.get("data").unwrap_or(&json);
                            let embed = embeds::build_analysis_embed_from_json(&symbol, data);
                            let signal = parse_signal_from_json(data);

                            // Try to generate chart
                            let now = chrono::Utc::now();
                            let start = now - chrono::Duration::days(90);
                            match self.polygon_client.get_aggregates(&symbol, 1, "day", start, now).await {
                                Ok(bars) if !bars.is_empty() => {
                                    match charts::generate_chart(&symbol, &bars, &signal).await {
                                        Ok(chart_path) => {
                                            if let Ok(attachment) = CreateAttachment::path(&chart_path).await {
                                                let builder = CreateMessage::default()
                                                    .embed(embed)
                                                    .add_file(attachment);
                                                let _ = msg.channel_id.send_message(&ctx.http, builder).await;
                                                let _ = std::fs::remove_file(chart_path);
                                            } else {
                                                let builder = CreateMessage::default().embed(embed);
                                                let _ = msg.channel_id.send_message(&ctx.http, builder).await;
                                            }
                                        }
                                        Err(_) => {
                                            let builder = CreateMessage::default().embed(embed);
                                            let _ = msg.channel_id.send_message(&ctx.http, builder).await;
                                        }
                                    }
                                }
                                _ => {
                                    let builder = CreateMessage::default().embed(embed);
                                    let _ = msg.channel_id.send_message(&ctx.http, builder).await;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = msg.channel_id.say(&ctx.http, format!("Error analyzing {}: {}", symbol, e)).await;
                        }
                    }
                }
                Ok(resp) => {
                    let _ = msg.channel_id.say(&ctx.http, format!("Error analyzing {}: {}", symbol, resp.status())).await;
                }
                Err(e) => {
                    let _ = msg.channel_id.say(&ctx.http, format!("Error analyzing {}: {}", symbol, e)).await;
                }
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!("{} is connected and ready!", ready.user.name);

        match Command::set_global_commands(&ctx.http, vec![create_iq_command()]).await {
            Ok(commands) => {
                tracing::info!("Registered {} global slash commands", commands.len());
            }
            Err(e) => {
                tracing::error!("Failed to register slash commands: {}", e);
            }
        }
    }
}

// === Command Handlers ===

impl Handler {
    /// Send a GET request to the API server, respecting the circuit breaker.
    async fn api_get(&self, url: &str) -> Result<reqwest::Response, String> {
        if self.circuit_breaker.is_open() {
            return Err("API server circuit breaker is open — skipping request".to_string());
        }
        match self.http_client.get(url).send().await {
            Ok(resp) => {
                if resp.status().is_server_error() {
                    self.circuit_breaker.record_failure();
                } else {
                    self.circuit_breaker.record_success();
                }
                Ok(resp)
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                Err(format!("{}", e))
            }
        }
    }

    async fn handle_analyze(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        subcommand: &CommandDataOption,
    ) {
        let symbol = get_string_opt(subcommand, "symbol");
        let Some(symbol) = symbol else {
            let _ = respond_ephemeral(ctx, command, "Please provide a symbol.").await;
            return;
        };
        let symbol = symbol.to_uppercase();

        let _ = command.defer(&ctx.http).await;

        // Use API server for analysis (gets all enhancements: ML weights, supplementary signals, etc.)
        let url = format!("{}/api/analyze/{}", self.api_base, symbol);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_analysis_embed_from_json(&symbol, data);

                        // Fetch bars for chart via Polygon directly (lightweight)
                        let chart_path = async {
                            let now = chrono::Utc::now();
                            let start = now - chrono::Duration::days(90);
                            let bars = self.polygon_client
                                .get_aggregates(&symbol, 1, "day", start, now)
                                .await
                                .ok()?;
                            if bars.is_empty() {
                                return None;
                            }
                            let signal = parse_signal_from_json(data);
                            charts::generate_chart(&symbol, &bars, &signal)
                                .await
                                .ok()
                        }
                        .await;

                        let mut response = EditInteractionResponse::new().embed(embed);

                        if let Some(ref path) = chart_path {
                            if let Ok(attachment) = CreateAttachment::path(path).await {
                                response = response.new_attachment(attachment);
                            }
                        }

                        let _ = command.edit_response(&ctx.http, response).await;

                        if let Some(path) = chart_path {
                            let _ = std::fs::remove_file(path);
                        }
                    }
                    Err(e) => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .content(format!("Error parsing analysis for {}: {}", symbol, e)),
                            )
                            .await;
                    }
                }
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content(format!("Error analyzing {} ({}): {}", symbol, status, &body[..body.len().min(200)])),
                    )
                    .await;
            }
            Err(e) => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content(format!("Error analyzing {}: {}", symbol, e)),
                    )
                    .await;
            }
        }
    }

    async fn handle_price(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        subcommand: &CommandDataOption,
    ) {
        let symbol = get_string_opt(subcommand, "symbol");
        let Some(symbol) = symbol else {
            let _ = respond_ephemeral(ctx, command, "Please provide a symbol.").await;
            return;
        };
        let symbol = symbol.to_uppercase();

        let _ = command.defer(&ctx.http).await;

        match self.polygon_client.get_snapshot(&symbol).await {
            Ok(snapshot) => {
                let embed = embeds::build_price_embed(&symbol, &snapshot);
                let _ = command
                    .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
                    .await;
            }
            Err(e) => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content(format!("Error fetching price for {}: {}", symbol, e)),
                    )
                    .await;
            }
        }
    }

    async fn handle_chart(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        subcommand: &CommandDataOption,
    ) {
        let symbol = get_string_opt(subcommand, "symbol");
        let Some(symbol) = symbol else {
            let _ = respond_ephemeral(ctx, command, "Please provide a symbol.").await;
            return;
        };
        let symbol = symbol.to_uppercase();
        let days = get_int_opt(subcommand, "days").unwrap_or(90);

        let _ = command.defer(&ctx.http).await;

        let now = chrono::Utc::now();
        let start = now - chrono::Duration::days(days);
        match self.polygon_client.get_aggregates(&symbol, 1, "day", start, now).await {
            Ok(bars) if !bars.is_empty() => {
                // Get signal from API server (via circuit breaker)
                let signal = match self.api_get(&format!("{}/api/analyze/{}", self.api_base, symbol)).await {
                    Ok(resp) => {
                        resp.json::<serde_json::Value>().await.ok()
                            .and_then(|json| {
                                let data = json.get("data").unwrap_or(&json);
                                Some(parse_signal_from_json(data))
                            })
                            .unwrap_or(SignalStrength::Neutral)
                    }
                    Err(_) => SignalStrength::Neutral,
                };

                match charts::generate_chart(&symbol, &bars, &signal).await {
                    Ok(chart_path) => {
                        if let Ok(attachment) = CreateAttachment::path(&chart_path).await {
                            let _ = command
                                .edit_response(
                                    &ctx.http,
                                    EditInteractionResponse::new().new_attachment(attachment),
                                )
                                .await;
                            let _ = std::fs::remove_file(chart_path);
                        } else {
                            let _ = command
                                .edit_response(
                                    &ctx.http,
                                    EditInteractionResponse::new()
                                        .content("Failed to upload chart."),
                                )
                                .await;
                        }
                    }
                    Err(e) => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .content(format!("Error generating chart: {}", e)),
                            )
                            .await;
                    }
                }
            }
            Ok(_) => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new().content("No bar data available."),
                    )
                    .await;
            }
            Err(e) => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content(format!("Error fetching data: {}", e)),
                    )
                    .await;
            }
        }
    }

    async fn handle_watchlist(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        subcommand: &CommandDataOption,
    ) {
        let action = get_string_opt(subcommand, "action").unwrap_or("show".to_string());
        let symbol = get_string_opt(subcommand, "symbol").map(|s| s.to_uppercase());
        let user_id = command.user.id;

        match action.as_str() {
            "add" => {
                let Some(sym) = symbol else {
                    let _ =
                        respond_ephemeral(ctx, command, "Please provide a symbol to add.").await;
                    return;
                };

                let mut wl = self.watchlists.write().await;
                let list = wl.entry(user_id).or_default();

                if list.len() >= MAX_WATCHLIST_SIZE {
                    let _ = respond_ephemeral(
                        ctx,
                        command,
                        &format!("Watchlist full (max {} symbols).", MAX_WATCHLIST_SIZE),
                    )
                    .await;
                    return;
                }

                if list.contains(&sym) {
                    let _ = respond_ephemeral(
                        ctx,
                        command,
                        &format!("{} is already in your watchlist.", sym),
                    )
                    .await;
                    return;
                }

                list.push(sym.clone());
                let _ = respond_ephemeral(
                    ctx,
                    command,
                    &format!(
                        "Added {} to your watchlist ({}/{}).",
                        sym,
                        list.len(),
                        MAX_WATCHLIST_SIZE
                    ),
                )
                .await;
            }
            "remove" => {
                let Some(sym) = symbol else {
                    let _ = respond_ephemeral(ctx, command, "Please provide a symbol to remove.")
                        .await;
                    return;
                };

                let mut wl = self.watchlists.write().await;
                if let Some(list) = wl.get_mut(&user_id) {
                    if let Some(pos) = list.iter().position(|s| s == &sym) {
                        list.remove(pos);
                        let _ = respond_ephemeral(
                            ctx,
                            command,
                            &format!("Removed {} from your watchlist.", sym),
                        )
                        .await;
                    } else {
                        let _ = respond_ephemeral(
                            ctx,
                            command,
                            &format!("{} is not in your watchlist.", sym),
                        )
                        .await;
                    }
                } else {
                    let _ = respond_ephemeral(ctx, command, "Your watchlist is empty.").await;
                }
            }
            _ => {
                // "show" or default
                let wl = self.watchlists.read().await;
                let list = wl.get(&user_id);

                if list.is_none() || list.is_some_and(|l| l.is_empty()) {
                    let _ = respond_ephemeral(
                        ctx,
                        command,
                        "Your watchlist is empty. Use `/iq watchlist add <symbol>` to add symbols.",
                    )
                    .await;
                    return;
                }

                let symbols = list.unwrap().clone();
                drop(wl);

                let _ = command.defer(&ctx.http).await;

                let mut embed = CreateEmbed::new()
                    .title(format!("Watchlist ({} symbols)", symbols.len()))
                    .color(0x3498DB)
                    .footer(serenity::builder::CreateEmbedFooter::new("InvestIQ"))
                    .timestamp(serenity::model::Timestamp::now());

                let futs: Vec<_> = symbols
                    .iter()
                    .map(|sym| {
                        let client = &self.polygon_client;
                        let sym = sym.clone();
                        async move {
                            let result = client.get_snapshot(&sym).await;
                            (sym, result)
                        }
                    })
                    .collect();

                let results = futures::future::join_all(futs).await;

                for (sym, result) in results {
                    match result {
                        Ok(snap) => {
                            let price = snap
                                .last_trade
                                .as_ref()
                                .and_then(|t| t.p)
                                .or_else(|| snap.day.as_ref().and_then(|d| d.c))
                                .unwrap_or(0.0);
                            let change_pct = snap.todays_change_perc.unwrap_or(0.0);
                            let arrow = if change_pct >= 0.0 { "+" } else { "" };
                            embed = embed.field(
                                sym,
                                format!("${:.2} ({}{:.2}%)", price, arrow, change_pct),
                                true,
                            );
                        }
                        Err(_) => {
                            embed = embed.field(sym, "N/A", true);
                        }
                    }
                }

                let _ = command
                    .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
                    .await;
            }
        }
    }

    async fn handle_portfolio(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let api_base = &self.api_base;

        let account_url = format!("{}/api/broker/account", api_base);
        let positions_url = format!("{}/api/broker/positions", api_base);
        let (account_res, positions_res) = tokio::join!(
            self.api_get(&account_url),
            self.api_get(&positions_url)
        );

        let account = match account_res {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => json.get("data").cloned().unwrap_or(json),
                    Err(e) => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .content(format!("Error parsing account: {}", e)),
                            )
                            .await;
                        return;
                    }
                }
            }
            Ok(resp) => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content(format!("API error: {}", resp.status())),
                    )
                    .await;
                return;
            }
            Err(e) => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content(format!("Error fetching account: {}", e)),
                    )
                    .await;
                return;
            }
        };

        let positions: Vec<serde_json::Value> = match positions_res {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => json
                        .get("data")
                        .and_then(|d| d.as_array())
                        .cloned()
                        .unwrap_or_default(),
                    Err(_) => Vec::new(),
                }
            }
            _ => Vec::new(),
        };

        let embed = embeds::build_portfolio_embed(&account, &positions);
        let _ = command
            .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
            .await;
    }

    async fn handle_compare(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        subcommand: &CommandDataOption,
    ) {
        let sym1 = get_string_opt(subcommand, "symbol1");
        let sym2 = get_string_opt(subcommand, "symbol2");

        let (Some(sym1), Some(sym2)) = (sym1, sym2) else {
            let _ = respond_ephemeral(ctx, command, "Please provide two symbols.").await;
            return;
        };

        let sym1 = sym1.to_uppercase();
        let sym2 = sym2.to_uppercase();

        let _ = command.defer(&ctx.http).await;

        let url1 = format!("{}/api/analyze/{}", self.api_base, sym1);
        let url2 = format!("{}/api/analyze/{}", self.api_base, sym2);

        let (res1, res2) = tokio::join!(
            self.api_get(&url1),
            self.api_get(&url2)
        );

        let mut embed_list: Vec<CreateEmbed> = Vec::new();

        match res1 {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        embed_list.push(embeds::build_compare_embed_from_json(&sym1, data));
                    }
                    Err(e) => {
                        embed_list.push(
                            CreateEmbed::new()
                                .title(format!("{} - Error", sym1))
                                .description(format!("{}", e))
                                .color(0xFF0000),
                        );
                    }
                }
            }
            Ok(resp) => {
                embed_list.push(
                    CreateEmbed::new()
                        .title(format!("{} - Error", sym1))
                        .description(format!("API error: {}", resp.status()))
                        .color(0xFF0000),
                );
            }
            Err(e) => {
                embed_list.push(
                    CreateEmbed::new()
                        .title(format!("{} - Error", sym1))
                        .description(format!("{}", e))
                        .color(0xFF0000),
                );
            }
        }

        match res2 {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        embed_list.push(embeds::build_compare_embed_from_json(&sym2, data));
                    }
                    Err(e) => {
                        embed_list.push(
                            CreateEmbed::new()
                                .title(format!("{} - Error", sym2))
                                .description(format!("{}", e))
                                .color(0xFF0000),
                        );
                    }
                }
            }
            Ok(resp) => {
                embed_list.push(
                    CreateEmbed::new()
                        .title(format!("{} - Error", sym2))
                        .description(format!("API error: {}", resp.status()))
                        .color(0xFF0000),
                );
            }
            Err(e) => {
                embed_list.push(
                    CreateEmbed::new()
                        .title(format!("{} - Error", sym2))
                        .description(format!("{}", e))
                        .color(0xFF0000),
                );
            }
        }

        let mut response = EditInteractionResponse::new();
        for embed in embed_list {
            response = response.add_embed(embed);
        }

        let _ = command.edit_response(&ctx.http, response).await;
    }

    async fn handle_backtest(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        subcommand: &CommandDataOption,
    ) {
        let symbol = get_string_opt(subcommand, "symbol");
        let Some(symbol) = symbol else {
            let _ = respond_ephemeral(ctx, command, "Please provide a symbol.").await;
            return;
        };
        let symbol = symbol.to_uppercase();
        let days = get_int_opt(subcommand, "days").unwrap_or(365);

        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/backtest/{}?days={}", self.api_base, symbol, days);

        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_backtest_embed(&symbol, data);
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new().embed(embed),
                            )
                            .await;
                    }
                    Err(e) => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .content(format!("Error parsing backtest: {}", e)),
                            )
                            .await;
                    }
                }
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content(format!("Backtest API error ({}): {}", status, body)),
                    )
                    .await;
            }
            Err(e) => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content(format!("Error running backtest: {}", e)),
                    )
                    .await;
            }
        }
    }

    async fn handle_help(&self, ctx: &Context, command: &CommandInteraction) {
        let embed = embeds::build_help_embed();
        let msg = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed),
        );
        let _ = command.create_response(&ctx.http, msg).await;
    }
}

// === Slash Command Registration ===

fn create_iq_command() -> CreateCommand {
    CreateCommand::new("iq")
        .description("InvestIQ stock analysis bot")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "analyze",
                "Full 4-engine analysis with chart",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "symbol",
                    "Stock ticker symbol (e.g. AAPL)",
                )
                .required(true),
            ),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "price",
                "Quick price snapshot",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "symbol",
                    "Stock ticker symbol",
                )
                .required(true),
            ),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "chart",
                "Enhanced technical chart",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "symbol",
                    "Stock ticker symbol",
                )
                .required(true),
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "days",
                    "Number of days (default 90)",
                )
                .required(false)
                .min_int_value(7)
                .max_int_value(365),
            ),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "watchlist",
                "Manage your personal watchlist",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "action",
                    "Action to perform",
                )
                .required(false)
                .add_string_choice("Show watchlist", "show")
                .add_string_choice("Add symbol", "add")
                .add_string_choice("Remove symbol", "remove"),
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "symbol",
                    "Symbol to add/remove",
                )
                .required(false),
            ),
        )
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "portfolio",
            "View paper trading portfolio",
        ))
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "compare",
                "Side-by-side analysis comparison",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "symbol1",
                    "First stock ticker",
                )
                .required(true),
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "symbol2",
                    "Second stock ticker",
                )
                .required(true),
            ),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "backtest",
                "Run backtest and show results",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "symbol",
                    "Stock ticker symbol",
                )
                .required(true),
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "days",
                    "Backtest period in days (default 365)",
                )
                .required(false)
                .min_int_value(30)
                .max_int_value(1825),
            ),
        )
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "help",
            "Show command reference",
        ))
}

// === Utility Functions ===

fn get_sub_options(opt: &CommandDataOption) -> &[CommandDataOption] {
    match &opt.value {
        CommandDataOptionValue::SubCommand(opts) => opts,
        _ => &[],
    }
}

fn get_string_opt(subcommand: &CommandDataOption, name: &str) -> Option<String> {
    for opt in get_sub_options(subcommand) {
        if opt.name == name {
            if let CommandDataOptionValue::String(s) = &opt.value {
                return Some(s.clone());
            }
        }
    }
    None
}

fn get_int_opt(subcommand: &CommandDataOption, name: &str) -> Option<i64> {
    for opt in get_sub_options(subcommand) {
        if opt.name == name {
            if let CommandDataOptionValue::Integer(v) = &opt.value {
                return Some(*v);
            }
        }
    }
    None
}

async fn respond_ephemeral(
    ctx: &Context,
    command: &CommandInteraction,
    content: &str,
) -> serenity::Result<()> {
    let msg = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(content)
            .ephemeral(true),
    );
    command.create_response(&ctx.http, msg).await
}

/// Parse signal strength from API JSON response
fn parse_signal_from_json(data: &serde_json::Value) -> SignalStrength {
    data.get("overall_signal")
        .and_then(|s| s.as_str())
        .map(|s| match s {
            "StrongBuy" => SignalStrength::StrongBuy,
            "Buy" => SignalStrength::Buy,
            "WeakBuy" => SignalStrength::WeakBuy,
            "WeakSell" => SignalStrength::WeakSell,
            "Sell" => SignalStrength::Sell,
            "StrongSell" => SignalStrength::StrongSell,
            _ => SignalStrength::Neutral,
        })
        .unwrap_or(SignalStrength::Neutral)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let json_logging = std::env::var("RUST_LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "discord_bot=info".into());

    if json_logging {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    let discord_token =
        std::env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN must be set");
    let polygon_api_key =
        std::env::var("POLYGON_API_KEY").expect("POLYGON_API_KEY must be set");

    let api_base = std::env::var("API_BASE_URL").unwrap_or_else(|_| API_BASE_URL.to_string());
    let polygon_client = Arc::new(polygon_client::PolygonClient::new(polygon_api_key));

    // Build HTTP client with API key for authenticated requests to the API server
    // Reads API_KEY first, falls back to the first key from API_KEYS (comma-separated)
    let mut default_headers = reqwest::header::HeaderMap::new();
    let api_key = std::env::var("API_KEY").ok().or_else(|| {
        std::env::var("API_KEYS").ok().and_then(|keys| {
            keys.split(',')
                .next()
                .map(|k| k.split(':').next().unwrap_or(k).trim().to_string())
        })
    });
    if let Some(key) = api_key {
        tracing::info!("API authentication configured for API server requests");
        default_headers.insert(
            reqwest::header::HeaderName::from_static("x-api-key"),
            reqwest::header::HeaderValue::from_str(&key).expect("Invalid API key value"),
        );
    } else {
        tracing::warn!("No API_KEY or API_KEYS set — API server requests will be unauthenticated");
    }
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .default_headers(default_headers)
        .build()?;

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let handler = Handler {
        http_client,
        api_base,
        polygon_client,
        watchlists: Arc::new(RwLock::new(HashMap::new())),
        rate_limits: Arc::new(RwLock::new(HashMap::new())),
        circuit_breaker: Arc::new(CircuitBreaker::new()),
    };

    let mut client = Client::builder(&discord_token, intents)
        .event_handler(handler)
        .await?;

    tracing::info!("Discord bot starting with slash commands...");

    // Graceful shutdown: SIGINT + SIGTERM
    let shard_manager = client.shard_manager.clone();
    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate())?;

    tokio::select! {
        result = client.start() => {
            if let Err(e) = result {
                tracing::error!("Discord client error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT — shutting down Discord bot...");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM — shutting down Discord bot...");
        }
    }

    shard_manager.shutdown_all().await;
    tracing::info!("Discord bot shut down.");

    Ok(())
}
