use crate::embeds;
use crate::Handler;
use crate::respond_ephemeral;
use super::{get_string_opt, resolve_subcommand};

use serenity::all::{
    CommandDataOption, CommandInteraction, EditInteractionResponse,
};
use serenity::prelude::*;

impl Handler {
    pub async fn handle_system(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        group_opt: &CommandDataOption,
    ) {
        let Some((sub_name, sub_opt)) = resolve_subcommand(group_opt) else {
            let _ = respond_ephemeral(ctx, command, "Unknown system subcommand.").await;
            return;
        };

        match sub_name {
            "health" => self.handle_system_health(ctx, command).await,
            "search" => self.handle_system_search(ctx, command, sub_opt).await,
            _ => {
                let _ = respond_ephemeral(ctx, command, "Unknown system subcommand.").await;
            }
        }
    }

    async fn handle_system_health(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/health", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) => {
                let status_code = resp.status();
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let embed = embeds::build_health_embed(status_code.as_u16(), &json);
                        let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().embed(embed)).await;
                    }
                    Err(e) => {
                        let _ = command.edit_response(&ctx.http,
                            EditInteractionResponse::new().content(format!("Error parsing health: {}", e))).await;
                    }
                }
            }
            Err(e) => {
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("Error: {}", e))).await;
            }
        }
    }

    async fn handle_system_search(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        sub_opt: &CommandDataOption,
    ) {
        let Some(query) = get_string_opt(sub_opt, "query") else {
            let _ = respond_ephemeral(ctx, command, "Please provide a search query.").await;
            return;
        };

        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/symbols/search?q={}&limit=10", self.api_base, urlencoding::encode(&query));
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data")
                            .and_then(|d| d.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let embed = embeds::build_search_embed(&query, &data);
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
