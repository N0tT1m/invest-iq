mod charts;
mod commands;
mod embeds;

use analysis_core::SignalStrength;
use serenity::{
    all::{
        Command, CommandInteraction, CommandOptionType,
        CreateCommand, CreateCommandOption, CreateInteractionResponse,
        CreateInteractionResponseMessage, Interaction,
    },
    async_trait,
    builder::{CreateAttachment, CreateMessage},
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

// ── Circuit Breaker ──────────────────────────────────────────────────

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

// ── Handler ──────────────────────────────────────────────────────────

pub(crate) struct Handler {
    pub(crate) http_client: reqwest::Client,
    pub(crate) api_base: String,
    pub(crate) polygon_client: Arc<polygon_client::PolygonClient>,
    pub(crate) watchlists: Arc<RwLock<HashMap<UserId, Vec<String>>>>,
    pub(crate) rate_limits: Arc<RwLock<HashMap<UserId, (Instant, u32)>>>,
    pub(crate) circuit_breaker: Arc<CircuitBreaker>,
    pub(crate) live_trading_key: Option<String>,
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

    /// GET request with circuit breaker.
    pub(crate) async fn api_get(&self, url: &str) -> Result<reqwest::Response, String> {
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

    /// POST request with circuit breaker.
    pub(crate) async fn api_post(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, String> {
        if self.circuit_breaker.is_open() {
            return Err("API server circuit breaker is open — skipping request".to_string());
        }
        match self.http_client.post(url).json(body).send().await {
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

    /// DELETE request with circuit breaker.
    #[allow(dead_code)]
    pub(crate) async fn api_delete(&self, url: &str) -> Result<reqwest::Response, String> {
        if self.circuit_breaker.is_open() {
            return Err("API server circuit breaker is open — skipping request".to_string());
        }
        match self.http_client.delete(url).send().await {
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

    /// POST request with X-Live-Trading-Key header.
    pub(crate) async fn api_post_with_trading_key(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, String> {
        if self.circuit_breaker.is_open() {
            return Err("API server circuit breaker is open — skipping request".to_string());
        }
        let mut req = self.http_client.post(url).json(body);
        if let Some(ref key) = self.live_trading_key {
            req = req.header("X-Live-Trading-Key", key);
        }
        match req.send().await {
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

    /// DELETE request with X-Live-Trading-Key header.
    pub(crate) async fn api_delete_with_trading_key(
        &self,
        url: &str,
    ) -> Result<reqwest::Response, String> {
        if self.circuit_breaker.is_open() {
            return Err("API server circuit breaker is open — skipping request".to_string());
        }
        let mut req = self.http_client.delete(url);
        if let Some(ref key) = self.live_trading_key {
            req = req.header("X-Live-Trading-Key", key);
        }
        match req.send().await {
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
}

// ── Event Handler ────────────────────────────────────────────────────

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
            // Existing top-level subcommands
            "analyze" => self.handle_analyze(&ctx, &command, subcommand).await,
            "price" => self.handle_price(&ctx, &command, subcommand).await,
            "chart" => self.handle_chart(&ctx, &command, subcommand).await,
            "watchlist" => self.handle_watchlist(&ctx, &command, subcommand).await,
            "portfolio" => self.handle_portfolio(&ctx, &command).await,
            "compare" => self.handle_compare(&ctx, &command, subcommand).await,
            "backtest" => self.handle_backtest(&ctx, &command, subcommand).await,
            "help" => self.handle_help(&ctx, &command).await,
            // New subcommand groups
            "agent" => self.handle_agent(&ctx, &command, subcommand).await,
            "risk" => self.handle_risk(&ctx, &command, subcommand).await,
            "market" => self.handle_market(&ctx, &command, subcommand).await,
            "data" => self.handle_data(&ctx, &command, subcommand).await,
            "trade" => self.handle_trade(&ctx, &command, subcommand).await,
            "tax" => self.handle_tax(&ctx, &command, subcommand).await,
            "advanced" => self.handle_advanced(&ctx, &command, subcommand).await,
            "system" => self.handle_system(&ctx, &command, subcommand).await,
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
                for cmd in &commands {
                    let sub_count = cmd.options.len();
                    tracing::info!("  /{} — {} subcommand(s)/group(s)", cmd.name, sub_count);
                }
            }
            Err(e) => {
                tracing::error!("Failed to register slash commands: {}", e);
            }
        }
    }
}

// ── Slash Command Registration ───────────────────────────────────────

fn create_iq_command() -> CreateCommand {
    CreateCommand::new("iq")
        .description("InvestIQ stock analysis bot")
        // ── Existing SubCommands ─────────────────────────
        .add_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "analyze", "Full 4-engine analysis with chart")
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "symbol", "Stock ticker symbol (e.g. AAPL)")
                        .required(true),
                ),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "price", "Quick price snapshot")
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "symbol", "Stock ticker symbol")
                        .required(true),
                ),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "chart", "Enhanced technical chart")
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "symbol", "Stock ticker symbol")
                        .required(true),
                )
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::Integer, "days", "Number of days (default 90)")
                        .required(false)
                        .min_int_value(7)
                        .max_int_value(365),
                ),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "watchlist", "Manage your personal watchlist")
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "action", "Action to perform")
                        .required(false)
                        .add_string_choice("Show watchlist", "show")
                        .add_string_choice("Add symbol", "add")
                        .add_string_choice("Remove symbol", "remove"),
                )
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "symbol", "Symbol to add/remove")
                        .required(false),
                ),
        )
        .add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "portfolio", "View paper trading portfolio"))
        .add_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "compare", "Side-by-side analysis comparison")
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "symbol1", "First stock ticker")
                        .required(true),
                )
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "symbol2", "Second stock ticker")
                        .required(true),
                ),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "backtest", "Run backtest and show results")
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "symbol", "Stock ticker symbol")
                        .required(true),
                )
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::Integer, "days", "Backtest period in days (default 365)")
                        .required(false)
                        .min_int_value(30)
                        .max_int_value(1825),
                ),
        )
        .add_option(CreateCommandOption::new(CommandOptionType::SubCommand, "help", "Show command reference"))
        // ── New SubCommandGroups ─────────────────────────
        .add_option(create_agent_group())
        .add_option(create_risk_group())
        .add_option(create_market_group())
        .add_option(create_data_group())
        .add_option(create_trade_group())
        .add_option(create_tax_group())
        .add_option(create_advanced_group())
        .add_option(create_system_group())
}

fn symbol_option(desc: &str) -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::String, "symbol", desc).required(true)
}

fn create_agent_group() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommandGroup, "agent", "Agent oversight commands")
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "pending", "List pending agent trades"),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "approve", "Approve a pending trade")
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "id", "Trade ID to approve")
                        .required(true),
                ),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "reject", "Reject a pending trade")
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "id", "Trade ID to reject")
                        .required(true),
                ),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "stats", "Agent analytics summary"),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "history", "Recent executed trades"),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "regimes", "Win rate by market regime"),
        )
}

fn create_risk_group() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommandGroup, "risk", "Risk management commands")
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "overview", "Portfolio risk radar"),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "symbol", "Per-symbol risk analysis")
                .add_sub_option(symbol_option("Stock ticker symbol")),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "breakers", "Circuit breaker status"),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "positions", "Stop-loss tracked positions"),
        )
}

fn create_market_group() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommandGroup, "market", "Market intelligence commands")
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "news", "Headlines and sentiment")
                .add_sub_option(symbol_option("Stock ticker symbol")),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "sectors", "Sector rotation and flows"),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "macro", "Macro indicators and regime"),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "screen", "Stock screener opportunities"),
        )
}

fn create_data_group() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommandGroup, "data", "Supplementary data commands")
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "earnings", "Earnings data")
                .add_sub_option(symbol_option("Stock ticker symbol")),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "dividends", "Dividend data")
                .add_sub_option(symbol_option("Stock ticker symbol")),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "options", "Options flow data")
                .add_sub_option(symbol_option("Stock ticker symbol")),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "insiders", "Insider transactions")
                .add_sub_option(symbol_option("Stock ticker symbol")),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "short", "Short interest analysis")
                .add_sub_option(symbol_option("Stock ticker symbol")),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "correlation", "Correlation with benchmarks")
                .add_sub_option(symbol_option("Stock ticker symbol")),
        )
}

fn create_trade_group() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommandGroup, "trade", "Trade execution commands")
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "buy", "Buy shares")
                .add_sub_option(symbol_option("Stock ticker symbol"))
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::Integer, "shares", "Number of shares")
                        .required(true)
                        .min_int_value(1),
                ),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "sell", "Sell shares")
                .add_sub_option(symbol_option("Stock ticker symbol"))
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::Integer, "shares", "Number of shares")
                        .required(true)
                        .min_int_value(1),
                ),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "close", "Close entire position")
                .add_sub_option(symbol_option("Stock ticker symbol")),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "orders", "Recent orders list"),
        )
}

fn create_tax_group() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommandGroup, "tax", "Tax tools")
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "harvest", "Tax-loss harvesting opportunities"),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "summary", "Year-end tax summary")
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::Integer, "year", "Tax year (default current)")
                        .required(false)
                        .min_int_value(2020)
                        .max_int_value(2030),
                ),
        )
}

fn create_advanced_group() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommandGroup, "advanced", "Advanced backtesting")
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "walkforward", "Walk-forward validation")
                .add_sub_option(symbol_option("Stock ticker symbol"))
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::Integer, "days", "Period in days (default 365)")
                        .required(false)
                        .min_int_value(90)
                        .max_int_value(1825),
                ),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "montecarlo", "Monte Carlo simulation")
                .add_sub_option(symbol_option("Stock ticker symbol"))
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::Integer, "sims", "Number of simulations (default 1000)")
                        .required(false)
                        .min_int_value(100)
                        .max_int_value(10000),
                ),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "strategies", "Strategy health / alpha decay"),
        )
}

fn create_system_group() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommandGroup, "system", "System commands")
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "health", "Dependency health check"),
        )
        .add_sub_option(
            CreateCommandOption::new(CommandOptionType::SubCommand, "search", "Symbol search")
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "query", "Search query (e.g. 'apple')")
                        .required(true),
                ),
        )
}

// ── Utility Functions ────────────────────────────────────────────────

pub(crate) async fn respond_ephemeral(
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

pub(crate) fn parse_signal_from_json(data: &serde_json::Value) -> SignalStrength {
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

// ── Main ─────────────────────────────────────────────────────────────

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
    let live_trading_key = std::env::var("LIVE_TRADING_KEY").ok();

    if live_trading_key.is_some() {
        tracing::info!("Live trading key configured — trade commands enabled");
    } else {
        tracing::info!("No LIVE_TRADING_KEY — trade commands will be blocked");
    }

    // Build HTTP client with API key for authenticated requests
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
        live_trading_key,
    };

    let mut client = Client::builder(&discord_token, intents)
        .event_handler(handler)
        .await?;

    tracing::info!("Discord bot starting with 36 slash commands (8 existing + 28 new)...");

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
