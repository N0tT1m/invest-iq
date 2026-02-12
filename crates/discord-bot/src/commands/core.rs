use crate::charts;
use crate::embeds;
use crate::Handler;
use crate::respond_ephemeral;
use crate::parse_signal_from_json;
use super::{get_string_opt, get_int_opt};

use serenity::all::{
    CommandDataOption, CommandInteraction, EditInteractionResponse,
};
use serenity::builder::{CreateAttachment, CreateEmbed};
use serenity::prelude::*;

impl Handler {
    pub async fn handle_analyze(
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

        let url = format!("{}/api/analyze/{}", self.api_base, symbol);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_analysis_embed_from_json(&symbol, data);

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

    pub async fn handle_price(
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

    pub async fn handle_chart(
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
                let signal = match self.api_get(&format!("{}/api/analyze/{}", self.api_base, symbol)).await {
                    Ok(resp) => {
                        resp.json::<serde_json::Value>().await.ok()
                            .map(|json| {
                                let data = json.get("data").unwrap_or(&json);
                                parse_signal_from_json(data)
                            })
                            .unwrap_or(analysis_core::SignalStrength::Neutral)
                    }
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

    pub async fn handle_watchlist(
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

                if list.len() >= crate::MAX_WATCHLIST_SIZE {
                    let _ = respond_ephemeral(
                        ctx,
                        command,
                        &format!("Watchlist full (max {} symbols).", crate::MAX_WATCHLIST_SIZE),
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
                        crate::MAX_WATCHLIST_SIZE
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

    pub async fn handle_portfolio(&self, ctx: &Context, command: &CommandInteraction) {
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

    pub async fn handle_compare(
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
                                .description(e.to_string())
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
                        .description(e.to_string())
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
                                .description(e.to_string())
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
                        .description(e.to_string())
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

    pub async fn handle_backtest(
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

    pub async fn handle_help(&self, ctx: &Context, command: &CommandInteraction) {
        let embed = embeds::build_help_embed();
        let msg = serenity::all::CreateInteractionResponse::Message(
            serenity::all::CreateInteractionResponseMessage::new().embed(embed),
        );
        let _ = command.create_response(&ctx.http, msg).await;
    }
}
