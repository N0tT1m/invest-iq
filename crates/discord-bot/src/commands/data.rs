use super::{get_string_opt, resolve_subcommand};
use crate::embeds;
use crate::respond_ephemeral;
use crate::Handler;

use serenity::all::{CommandDataOption, CommandInteraction, EditInteractionResponse};
use serenity::prelude::*;

impl Handler {
    pub async fn handle_data(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        group_opt: &CommandDataOption,
    ) {
        let Some((sub_name, sub_opt)) = resolve_subcommand(group_opt) else {
            let _ = respond_ephemeral(ctx, command, "Unknown data subcommand.").await;
            return;
        };

        match sub_name {
            "earnings" => {
                self.handle_data_endpoint(ctx, command, sub_opt, "earnings")
                    .await
            }
            "dividends" => {
                self.handle_data_endpoint(ctx, command, sub_opt, "dividends")
                    .await
            }
            "options" => {
                self.handle_data_endpoint(ctx, command, sub_opt, "options")
                    .await
            }
            "insiders" => {
                self.handle_data_endpoint(ctx, command, sub_opt, "insiders")
                    .await
            }
            "short" => {
                self.handle_data_endpoint(ctx, command, sub_opt, "short-interest")
                    .await
            }
            "correlation" => {
                self.handle_data_endpoint(ctx, command, sub_opt, "correlation")
                    .await
            }
            _ => {
                let _ = respond_ephemeral(ctx, command, "Unknown data subcommand.").await;
            }
        }
    }

    async fn handle_data_endpoint(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        sub_opt: &CommandDataOption,
        endpoint: &str,
    ) {
        let Some(symbol) = get_string_opt(sub_opt, "symbol") else {
            let _ = respond_ephemeral(ctx, command, "Please provide a symbol.").await;
            return;
        };
        let symbol = symbol.to_uppercase();

        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/{}/{}", self.api_base, endpoint, symbol);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = match endpoint {
                            "earnings" => embeds::build_earnings_embed(&symbol, data),
                            "dividends" => embeds::build_dividends_embed(&symbol, data),
                            "options" => embeds::build_options_embed(&symbol, data),
                            "insiders" => embeds::build_insiders_embed(&symbol, data),
                            "short-interest" => embeds::build_short_interest_embed(&symbol, data),
                            "correlation" => embeds::build_correlation_embed(&symbol, data),
                            _ => unreachable!(),
                        };
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
