use super::{get_string_opt, resolve_subcommand};
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
    pub async fn handle_agent(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        group_opt: &CommandDataOption,
    ) {
        let Some((sub_name, sub_opt)) = resolve_subcommand(group_opt) else {
            let _ = respond_ephemeral(ctx, command, "Unknown agent subcommand.").await;
            return;
        };

        match sub_name {
            "pending" => self.handle_agent_pending(ctx, command).await,
            "approve" => self.handle_agent_approve(ctx, command, sub_opt).await,
            "reject" => self.handle_agent_reject(ctx, command, sub_opt).await,
            "stats" => self.handle_agent_stats(ctx, command).await,
            "history" => self.handle_agent_history(ctx, command).await,
            "regimes" => self.handle_agent_regimes(ctx, command).await,
            _ => {
                let _ = respond_ephemeral(ctx, command, "Unknown agent subcommand.").await;
            }
        }
    }

    async fn handle_agent_pending(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/agent/trades", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let trades = json
                            .get("data")
                            .and_then(|d| d.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let pending: Vec<_> = trades
                            .iter()
                            .filter(|t| t.get("status").and_then(|s| s.as_str()) == Some("pending"))
                            .collect();
                        let embed = embeds::build_agent_pending_embed(&pending);
                        let _ = command
                            .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
                            .await;
                    }
                    Err(e) => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .content(format!("Error parsing trades: {}", e)),
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

    async fn handle_agent_approve(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        sub_opt: &CommandDataOption,
    ) {
        let Some(id_str) = get_string_opt(sub_opt, "id") else {
            let _ = respond_ephemeral(ctx, command, "Please provide a trade ID.").await;
            return;
        };

        // First fetch the trade details
        let detail_url = format!("{}/api/agent/trades", self.api_base);
        let trade_info = match self.api_get(&detail_url).await {
            Ok(resp) if resp.status().is_success() => resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|json| {
                    json.get("data")
                        .and_then(|d| d.as_array())
                        .and_then(|arr| {
                            arr.iter().find(|t| {
                                t.get("id").and_then(|i| i.as_i64()).map(|i| i.to_string())
                                    == Some(id_str.clone())
                            })
                        })
                        .cloned()
                }),
            _ => None,
        };

        let desc = if let Some(ref trade) = trade_info {
            let sym = trade
                .get("symbol")
                .and_then(|v| v.as_str())
                .unwrap_or("???");
            let action = trade
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("???");
            let qty = trade
                .get("quantity")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            format!("**{}** {} x {:.1} shares", action.to_uppercase(), sym, qty)
        } else {
            format!("Trade #{}", id_str)
        };

        // Show confirmation button
        let confirm_id = format!("agent_approve_{}_{}", id_str, command.id);
        let cancel_id = format!("agent_cancel_{}_{}", id_str, command.id);

        let components = vec![CreateActionRow::Buttons(vec![
            CreateButton::new(&confirm_id)
                .label("Approve")
                .style(serenity::all::ButtonStyle::Success),
            CreateButton::new(&cancel_id)
                .label("Cancel")
                .style(serenity::all::ButtonStyle::Secondary),
        ])];

        let msg = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(
                    CreateEmbed::new()
                        .title("Approve Agent Trade?")
                        .description(&desc)
                        .color(Colour::from(0xFFD700)),
                )
                .components(components)
                .ephemeral(true),
        );
        let _ = command.create_response(&ctx.http, msg).await;

        // Await button click
        let interaction = ComponentInteractionCollector::new(&ctx.shard)
            .filter(move |i| {
                matches!(&i.data.kind, ComponentInteractionDataKind::Button)
                    && (i.data.custom_id == confirm_id || i.data.custom_id == cancel_id)
            })
            .timeout(Duration::from_secs(30))
            .await;

        match interaction {
            Some(interaction) if interaction.data.custom_id.starts_with("agent_approve_") => {
                let _ = interaction.defer(&ctx.http).await;
                let review_url = format!("{}/api/agent/trades/{}/review", self.api_base, id_str);
                let body = serde_json::json!({"action": "approve"});
                match self.api_post(&review_url, &body).await {
                    Ok(resp) if resp.status().is_success() => {
                        let _ = command
                            .edit_response(
                                &ctx.http,
                                EditInteractionResponse::new()
                                    .embed(
                                        CreateEmbed::new()
                                            .title("Trade Approved")
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
                                            .title("Approval Failed")
                                            .description(format!(
                                                "API error: {}",
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
                                            .title("Approval Failed")
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
                                    .description("Trade approval cancelled.")
                                    .color(Colour::from(0x808080)),
                            )
                            .components(vec![]),
                    )
                    .await;
            }
        }
    }

    async fn handle_agent_reject(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        sub_opt: &CommandDataOption,
    ) {
        let Some(id_str) = get_string_opt(sub_opt, "id") else {
            let _ = respond_ephemeral(ctx, command, "Please provide a trade ID.").await;
            return;
        };

        let _ = command.defer(&ctx.http).await;

        let review_url = format!("{}/api/agent/trades/{}/review", self.api_base, id_str);
        let body = serde_json::json!({"action": "reject"});
        match self.api_post(&review_url, &body).await {
            Ok(resp) if resp.status().is_success() => {
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new().embed(
                            CreateEmbed::new()
                                .title("Trade Rejected")
                                .description(format!("Trade #{} has been rejected.", id_str))
                                .color(Colour::from(0xFF0000)),
                        ),
                    )
                    .await;
            }
            Ok(resp) => {
                let body = resp.text().await.unwrap_or_default();
                let _ = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .content(format!("API error: {}", &body[..body.len().min(200)])),
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

    async fn handle_agent_stats(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/agent/analytics/summary", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_agent_stats_embed(data);
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

    async fn handle_agent_history(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/agent/trades", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let trades = json
                            .get("data")
                            .and_then(|d| d.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let executed: Vec<_> = trades
                            .iter()
                            .filter(|t| t.get("status").and_then(|s| s.as_str()) != Some("pending"))
                            .take(10)
                            .collect();
                        let embed = embeds::build_agent_history_embed(&executed);
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

    async fn handle_agent_regimes(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/agent/analytics/win-rate-by-regime", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_agent_regimes_embed(data);
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
