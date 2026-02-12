use super::{get_int_opt, get_string_opt, resolve_subcommand};
use crate::embeds;
use crate::respond_ephemeral;
use crate::Handler;

use serenity::all::{
    CommandDataOption, CommandInteraction, ComponentInteractionDataKind, CreateActionRow,
    CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse,
};
use serenity::builder::CreateEmbed;
use serenity::collector::ComponentInteractionCollector;
use serenity::model::colour::Colour;
use serenity::prelude::*;
use std::time::Duration;

impl Handler {
    pub async fn handle_trade(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        group_opt: &CommandDataOption,
    ) {
        let Some((sub_name, sub_opt)) = resolve_subcommand(group_opt) else {
            let _ = respond_ephemeral(ctx, command, "Unknown trade subcommand.").await;
            return;
        };

        match sub_name {
            "buy" => {
                self.handle_trade_execute(ctx, command, sub_opt, "buy")
                    .await
            }
            "sell" => {
                self.handle_trade_execute(ctx, command, sub_opt, "sell")
                    .await
            }
            "close" => self.handle_trade_close(ctx, command, sub_opt).await,
            "orders" => self.handle_trade_orders(ctx, command).await,
            _ => {
                let _ = respond_ephemeral(ctx, command, "Unknown trade subcommand.").await;
            }
        }
    }

    async fn handle_trade_execute(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        sub_opt: &CommandDataOption,
        side: &str,
    ) {
        // Check live trading key
        if self.live_trading_key.is_none() {
            let _ = respond_ephemeral(
                ctx,
                command,
                "Live trading key not configured. Set `LIVE_TRADING_KEY` env var.",
            )
            .await;
            return;
        }

        let Some(symbol) = get_string_opt(sub_opt, "symbol") else {
            let _ = respond_ephemeral(ctx, command, "Please provide a symbol.").await;
            return;
        };
        let symbol = symbol.to_uppercase();

        let Some(shares) = get_int_opt(sub_opt, "shares") else {
            let _ = respond_ephemeral(ctx, command, "Please provide number of shares.").await;
            return;
        };

        // Fetch current price for confirmation
        let price_info = match self.polygon_client.get_snapshot(&symbol).await {
            Ok(snap) => {
                let price = snap
                    .last_trade
                    .as_ref()
                    .and_then(|t| t.p)
                    .or_else(|| snap.day.as_ref().and_then(|d| d.c))
                    .unwrap_or(0.0);
                format!("~${:.2} (est. ${:.2} total)", price, price * shares as f64)
            }
            Err(_) => "Price unavailable".to_string(),
        };

        let desc = format!(
            "**{}** {} x {} shares\n{}",
            side.to_uppercase(),
            symbol,
            shares,
            price_info
        );

        let confirm_id = format!("trade_confirm_{}_{}_{}", side, symbol, command.id);
        let cancel_id = format!("trade_cancel_{}_{}_{}", side, symbol, command.id);

        let components = vec![CreateActionRow::Buttons(vec![
            CreateButton::new(&confirm_id)
                .label(format!(
                    "Confirm {}",
                    if side == "buy" { "Buy" } else { "Sell" }
                ))
                .style(if side == "buy" {
                    serenity::all::ButtonStyle::Success
                } else {
                    serenity::all::ButtonStyle::Danger
                }),
            CreateButton::new(&cancel_id)
                .label("Cancel")
                .style(serenity::all::ButtonStyle::Secondary),
        ])];

        let msg = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(
                    CreateEmbed::new()
                        .title(format!(
                            "Confirm {} Order",
                            if side == "buy" { "Buy" } else { "Sell" }
                        ))
                        .description(&desc)
                        .color(Colour::from(0xFFD700)),
                )
                .components(components)
                .ephemeral(true),
        );
        let _ = command.create_response(&ctx.http, msg).await;

        let confirm_id_clone = confirm_id.clone();
        let cancel_id_clone = cancel_id.clone();
        let interaction = ComponentInteractionCollector::new(&ctx.shard)
            .filter(move |i| {
                matches!(&i.data.kind, ComponentInteractionDataKind::Button)
                    && (i.data.custom_id == confirm_id_clone || i.data.custom_id == cancel_id_clone)
            })
            .timeout(Duration::from_secs(30))
            .await;

        match interaction {
            Some(interaction) if interaction.data.custom_id == confirm_id => {
                let _ = interaction.defer(&ctx.http).await;
                let execute_url = format!("{}/api/broker/execute", self.api_base);
                let body = serde_json::json!({
                    "symbol": symbol,
                    "side": side,
                    "qty": shares,
                    "order_type": "market"
                });
                match self.api_post_with_trading_key(&execute_url, &body).await {
                    Ok(resp) if resp.status().is_success() => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .embed(
                                        CreateEmbed::new()
                                            .title("Order Submitted")
                                            .description(desc)
                                            .color(Colour::from(0x00FF00)),
                                    )
                                    .components(vec![]),
                            )
                            .await;
                    }
                    Ok(resp) => {
                        let body = resp.text().await.unwrap_or_default();
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .embed(
                                        CreateEmbed::new()
                                            .title("Order Failed")
                                            .description(format!(
                                                "Error: {}",
                                                &body[..body.len().min(200)]
                                            ))
                                            .color(Colour::from(0xFF0000)),
                                    )
                                    .components(vec![]),
                            )
                            .await;
                    }
                    Err(e) => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .embed(
                                        CreateEmbed::new()
                                            .title("Order Failed")
                                            .description(format!("Error: {}", e))
                                            .color(Colour::from(0xFF0000)),
                                    )
                                    .components(vec![]),
                            )
                            .await;
                    }
                }
            }
            _ => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .embed(
                                CreateEmbed::new()
                                    .title("Cancelled")
                                    .description("Order cancelled.")
                                    .color(Colour::from(0x808080)),
                            )
                            .components(vec![]),
                    )
                    .await;
            }
        }
    }

    async fn handle_trade_close(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        sub_opt: &CommandDataOption,
    ) {
        if self.live_trading_key.is_none() {
            let _ = respond_ephemeral(ctx, command, "Live trading key not configured.").await;
            return;
        }

        let Some(symbol) = get_string_opt(sub_opt, "symbol") else {
            let _ = respond_ephemeral(ctx, command, "Please provide a symbol.").await;
            return;
        };
        let symbol = symbol.to_uppercase();

        let desc = format!("Close entire position in **{}**", symbol);
        let confirm_id = format!("close_confirm_{}_{}", symbol, command.id);
        let cancel_id = format!("close_cancel_{}_{}", symbol, command.id);

        let components = vec![CreateActionRow::Buttons(vec![
            CreateButton::new(&confirm_id)
                .label("Confirm Close")
                .style(serenity::all::ButtonStyle::Danger),
            CreateButton::new(&cancel_id)
                .label("Cancel")
                .style(serenity::all::ButtonStyle::Secondary),
        ])];

        let msg = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(
                    CreateEmbed::new()
                        .title("Close Position?")
                        .description(&desc)
                        .color(Colour::from(0xFFD700)),
                )
                .components(components)
                .ephemeral(true),
        );
        let _ = command.create_response(&ctx.http, msg).await;

        let confirm_id_clone = confirm_id.clone();
        let cancel_id_clone = cancel_id.clone();
        let interaction = ComponentInteractionCollector::new(&ctx.shard)
            .filter(move |i| {
                matches!(&i.data.kind, ComponentInteractionDataKind::Button)
                    && (i.data.custom_id == confirm_id_clone || i.data.custom_id == cancel_id_clone)
            })
            .timeout(Duration::from_secs(30))
            .await;

        match interaction {
            Some(interaction) if interaction.data.custom_id == confirm_id => {
                let _ = interaction.defer(&ctx.http).await;
                let close_url = format!("{}/api/broker/positions/{}", self.api_base, symbol);
                match self.api_delete_with_trading_key(&close_url).await {
                    Ok(resp) if resp.status().is_success() => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .embed(
                                        CreateEmbed::new()
                                            .title("Position Closed")
                                            .description(desc)
                                            .color(Colour::from(0x00FF00)),
                                    )
                                    .components(vec![]),
                            )
                            .await;
                    }
                    Ok(resp) => {
                        let body = resp.text().await.unwrap_or_default();
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .embed(
                                        CreateEmbed::new()
                                            .title("Close Failed")
                                            .description(format!(
                                                "Error: {}",
                                                &body[..body.len().min(200)]
                                            ))
                                            .color(Colour::from(0xFF0000)),
                                    )
                                    .components(vec![]),
                            )
                            .await;
                    }
                    Err(e) => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .embed(
                                        CreateEmbed::new()
                                            .title("Close Failed")
                                            .description(format!("Error: {}", e))
                                            .color(Colour::from(0xFF0000)),
                                    )
                                    .components(vec![]),
                            )
                            .await;
                    }
                }
            }
            _ => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .embed(
                                CreateEmbed::new()
                                    .title("Cancelled")
                                    .description("Close cancelled.")
                                    .color(Colour::from(0x808080)),
                            )
                            .components(vec![]),
                    )
                    .await;
            }
        }
    }

    async fn handle_trade_orders(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/broker/orders", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let orders = json
                            .get("data")
                            .and_then(|d| d.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let embed = embeds::build_orders_embed(&orders);
                        let _ = command
                            .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
                            .await;
                    }
                    Err(e) => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new().content(format!("Error: {}", e)),
                            )
                            .await;
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
            }
            Err(e) => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new().content(format!("Error: {}", e)),
                    )
                    .await;
            }
        }
    }
}
