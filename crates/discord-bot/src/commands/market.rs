use crate::embeds;
use crate::Handler;
use crate::respond_ephemeral;
use super::{get_string_opt, resolve_subcommand};

use serenity::all::{
    CommandDataOption, CommandInteraction, EditInteractionResponse,
};
use serenity::prelude::*;

impl Handler {
    pub async fn handle_market(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        group_opt: &CommandDataOption,
    ) {
        let Some((sub_name, sub_opt)) = resolve_subcommand(group_opt) else {
            let _ = respond_ephemeral(ctx, command, "Unknown market subcommand.").await;
            return;
        };

        match sub_name {
            "news" => self.handle_market_news(ctx, command, sub_opt).await,
            "sectors" => self.handle_market_sectors(ctx, command).await,
            "macro" => self.handle_market_macro(ctx, command).await,
            "screen" => self.handle_market_screen(ctx, command).await,
            _ => {
                let _ = respond_ephemeral(ctx, command, "Unknown market subcommand.").await;
            }
        }
    }

    async fn handle_market_news(
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

        let url = format!("{}/api/sentiment/{}/social", self.api_base, symbol);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_news_embed(&symbol, data);
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

    async fn handle_market_sectors(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/flows/sectors", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_sectors_embed(data);
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

    async fn handle_market_macro(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/macro/indicators", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_macro_embed(data);
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

    async fn handle_market_screen(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/watchlist/scan", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_screen_embed(data);
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
