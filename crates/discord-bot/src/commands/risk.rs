use crate::embeds;
use crate::Handler;
use crate::respond_ephemeral;
use super::{get_string_opt, resolve_subcommand};

use serenity::all::{
    CommandDataOption, CommandInteraction, EditInteractionResponse,
};
use serenity::prelude::*;

impl Handler {
    pub async fn handle_risk(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        group_opt: &CommandDataOption,
    ) {
        let Some((sub_name, sub_opt)) = resolve_subcommand(group_opt) else {
            let _ = respond_ephemeral(ctx, command, "Unknown risk subcommand.").await;
            return;
        };

        match sub_name {
            "overview" => self.handle_risk_overview(ctx, command).await,
            "symbol" => self.handle_risk_symbol(ctx, command, sub_opt).await,
            "breakers" => self.handle_risk_breakers(ctx, command).await,
            "positions" => self.handle_risk_positions(ctx, command).await,
            _ => {
                let _ = respond_ephemeral(ctx, command, "Unknown risk subcommand.").await;
            }
        }
    }

    async fn handle_risk_overview(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/risk/radar", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_risk_overview_embed(data);
                        let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().embed(embed)).await;
                    }
                    Err(e) => {
                        let _ = command.edit_response(&ctx.http,
                            EditInteractionResponse::new().content(format!("Error: {}", e))).await;
                    }
                }
            }
            Ok(resp) => {
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("API error: {}", resp.status()))).await;
            }
            Err(e) => {
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("Error: {}", e))).await;
            }
        }
    }

    async fn handle_risk_symbol(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        sub_opt: &CommandDataOption,
    ) {
        let Some(symbol) = get_string_opt(sub_opt, "symbol") else {
            let _ = respond_ephemeral(ctx, command, "Please provide a symbol.").await;
            return;
        };
        let symbol = symbol.to_uppercase();

        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/risk/radar/{}", self.api_base, symbol);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_risk_symbol_embed(&symbol, data);
                        let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().embed(embed)).await;
                    }
                    Err(e) => {
                        let _ = command.edit_response(&ctx.http,
                            EditInteractionResponse::new().content(format!("Error: {}", e))).await;
                    }
                }
            }
            Ok(resp) => {
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("API error: {}", resp.status()))).await;
            }
            Err(e) => {
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("Error: {}", e))).await;
            }
        }
    }

    async fn handle_risk_breakers(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/risk/circuit-breakers", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_circuit_breakers_embed(data);
                        let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().embed(embed)).await;
                    }
                    Err(e) => {
                        let _ = command.edit_response(&ctx.http,
                            EditInteractionResponse::new().content(format!("Error: {}", e))).await;
                    }
                }
            }
            Ok(resp) => {
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("API error: {}", resp.status()))).await;
            }
            Err(e) => {
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("Error: {}", e))).await;
            }
        }
    }

    async fn handle_risk_positions(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/risk/positions", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data")
                            .and_then(|d| d.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let embed = embeds::build_risk_positions_embed(&data);
                        let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().embed(embed)).await;
                    }
                    Err(e) => {
                        let _ = command.edit_response(&ctx.http,
                            EditInteractionResponse::new().content(format!("Error: {}", e))).await;
                    }
                }
            }
            Ok(resp) => {
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("API error: {}", resp.status()))).await;
            }
            Err(e) => {
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("Error: {}", e))).await;
            }
        }
    }
}
