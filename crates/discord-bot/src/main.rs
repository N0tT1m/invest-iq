mod charts;
mod embeds;

use analysis_core::Timeframe;
use analysis_orchestrator::AnalysisOrchestrator;
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
use std::time::Instant;
use tokio::sync::RwLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const API_BASE_URL: &str = "http://localhost:3000";
const RATE_LIMIT_COMMANDS: u32 = 5;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;
const MAX_WATCHLIST_SIZE: usize = 20;

struct Handler {
    orchestrator: Arc<AnalysisOrchestrator>,
    http_client: reqwest::Client,
    watchlists: Arc<RwLock<HashMap<UserId, Vec<String>>>>,
    rate_limits: Arc<RwLock<HashMap<UserId, (Instant, u32)>>>,
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

            match self
                .orchestrator
                .analyze(&symbol, Timeframe::Day1, 365)
                .await
            {
                Ok(analysis) => {
                    let embed = embeds::build_analysis_embed(&symbol, &analysis);
                    match self
                        .orchestrator
                        .get_bars(&symbol, Timeframe::Day1, 90)
                        .await
                    {
                        Ok(bars) if !bars.is_empty() => {
                            match charts::generate_chart(&symbol, &bars, &analysis.overall_signal)
                                .await
                            {
                                Ok(chart_path) => {
                                    if let Ok(attachment) =
                                        CreateAttachment::path(&chart_path).await
                                    {
                                        let builder = CreateMessage::default()
                                            .embed(embed)
                                            .add_file(attachment);
                                        let _ =
                                            msg.channel_id.send_message(&ctx.http, builder).await;
                                        let _ = std::fs::remove_file(chart_path);
                                    } else {
                                        let builder = CreateMessage::default().embed(embed);
                                        let _ =
                                            msg.channel_id.send_message(&ctx.http, builder).await;
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
                    let _ = msg
                        .channel_id
                        .say(&ctx.http, format!("Error analyzing {}: {}", symbol, e))
                        .await;
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

        match self
            .orchestrator
            .analyze(&symbol, Timeframe::Day1, 365)
            .await
        {
            Ok(analysis) => {
                let embed = embeds::build_analysis_embed(&symbol, &analysis);

                let chart_path = async {
                    let bars = self
                        .orchestrator
                        .get_bars(&symbol, Timeframe::Day1, 90)
                        .await
                        .ok()?;
                    if bars.is_empty() {
                        return None;
                    }
                    charts::generate_chart(&symbol, &bars, &analysis.overall_signal)
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

        match self
            .orchestrator
            .polygon_client
            .get_snapshot(&symbol)
            .await
        {
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

        match self
            .orchestrator
            .get_bars(&symbol, Timeframe::Day1, days)
            .await
        {
            Ok(bars) if !bars.is_empty() => {
                let signal = match self
                    .orchestrator
                    .analyze(&symbol, Timeframe::Day1, 365)
                    .await
                {
                    Ok(a) => a.overall_signal,
                    Err(_) => analysis_core::SignalStrength::Neutral,
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
                        let client = &self.orchestrator.polygon_client;
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

        let api_base =
            std::env::var("API_BASE_URL").unwrap_or_else(|_| API_BASE_URL.to_string());

        let account_fut = self
            .http_client
            .get(format!("{}/api/broker/account", api_base))
            .send();
        let positions_fut = self
            .http_client
            .get(format!("{}/api/broker/positions", api_base))
            .send();

        let (account_res, positions_res) = tokio::join!(account_fut, positions_fut);

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

        let (res1, res2) = tokio::join!(
            self.orchestrator.analyze(&sym1, Timeframe::Day1, 365),
            self.orchestrator.analyze(&sym2, Timeframe::Day1, 365)
        );

        let mut embed_list: Vec<CreateEmbed> = Vec::new();

        match res1 {
            Ok(a) => embed_list.push(embeds::build_compare_embed(&sym1, &a)),
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
            Ok(a) => embed_list.push(embeds::build_compare_embed(&sym2, &a)),
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

        let api_base =
            std::env::var("API_BASE_URL").unwrap_or_else(|_| API_BASE_URL.to_string());

        let url = format!("{}/api/backtest/{}?days={}", api_base, symbol, days);

        match self.http_client.get(&url).send().await {
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "discord_bot=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let discord_token =
        std::env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN must be set");
    let polygon_api_key =
        std::env::var("POLYGON_API_KEY").expect("POLYGON_API_KEY must be set");

    let orchestrator = Arc::new(AnalysisOrchestrator::new(polygon_api_key));

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let handler = Handler {
        orchestrator,
        http_client,
        watchlists: Arc::new(RwLock::new(HashMap::new())),
        rate_limits: Arc::new(RwLock::new(HashMap::new())),
    };

    let mut client = Client::builder(&discord_token, intents)
        .event_handler(handler)
        .await?;

    tracing::info!("Discord bot starting with slash commands...");

    client.start().await?;

    Ok(())
}
