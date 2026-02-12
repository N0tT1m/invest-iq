use chrono::Datelike;

use super::{get_int_opt, resolve_subcommand};
use crate::embeds;
use crate::respond_ephemeral;
use crate::Handler;

use serenity::all::{CommandDataOption, CommandInteraction, EditInteractionResponse};
use serenity::prelude::*;

impl Handler {
    pub async fn handle_tax(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        group_opt: &CommandDataOption,
    ) {
        let Some((sub_name, sub_opt)) = resolve_subcommand(group_opt) else {
            let _ = respond_ephemeral(ctx, command, "Unknown tax subcommand.").await;
            return;
        };

        match sub_name {
            "harvest" => self.handle_tax_harvest(ctx, command).await,
            "summary" => self.handle_tax_summary(ctx, command, sub_opt).await,
            _ => {
                let _ = respond_ephemeral(ctx, command, "Unknown tax subcommand.").await;
            }
        }
    }

    async fn handle_tax_harvest(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/tax/harvest-opportunities", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_tax_harvest_embed(data);
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

    async fn handle_tax_summary(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        sub_opt: &CommandDataOption,
    ) {
        let _ = command.defer(&ctx.http).await;

        let year = get_int_opt(sub_opt, "year").unwrap_or_else(|| chrono::Utc::now().year() as i64);

        let url = format!("{}/api/tax/year-end?year={}", self.api_base, year);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_tax_summary_embed(year, data);
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
