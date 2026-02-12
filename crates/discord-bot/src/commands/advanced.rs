use crate::embeds;
use crate::Handler;
use crate::respond_ephemeral;
use super::{get_string_opt, get_int_opt, resolve_subcommand};

use serenity::all::{
    CommandDataOption, CommandInteraction, EditInteractionResponse,
};
use serenity::prelude::*;

impl Handler {
    pub async fn handle_advanced(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        group_opt: &CommandDataOption,
    ) {
        let Some((sub_name, sub_opt)) = resolve_subcommand(group_opt) else {
            let _ = respond_ephemeral(ctx, command, "Unknown advanced subcommand.").await;
            return;
        };

        match sub_name {
            "walkforward" => self.handle_advanced_walkforward(ctx, command, sub_opt).await,
            "montecarlo" => self.handle_advanced_montecarlo(ctx, command, sub_opt).await,
            "strategies" => self.handle_advanced_strategies(ctx, command).await,
            _ => {
                let _ = respond_ephemeral(ctx, command, "Unknown advanced subcommand.").await;
            }
        }
    }

    async fn handle_advanced_walkforward(
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
        let days = get_int_opt(sub_opt, "days").unwrap_or(365);

        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/backtest/walk-forward", self.api_base);
        let body = serde_json::json!({
            "symbol": symbol,
            "days": days
        });
        match self.api_post(&url, &body).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_walkforward_embed(&symbol, data);
                        let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().embed(embed)).await;
                    }
                    Err(e) => {
                        let _ = command.edit_response(&ctx.http,
                            EditInteractionResponse::new().content(format!("Error: {}", e))).await;
                    }
                }
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("API error ({}): {}", status, &body[..body.len().min(200)]))).await;
            }
            Err(e) => {
                let _ = command.edit_response(&ctx.http,
                    EditInteractionResponse::new().content(format!("Error: {}", e))).await;
            }
        }
    }

    async fn handle_advanced_montecarlo(
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
        let sims = get_int_opt(sub_opt, "sims").unwrap_or(1000);

        let _ = command.defer(&ctx.http).await;

        // First run backtest to get results, then monte carlo
        let backtest_url = format!("{}/api/backtest/{}?days=365", self.api_base, symbol);
        let backtest_id = match self.api_get(&backtest_url).await {
            Ok(resp) if resp.status().is_success() => {
                resp.json::<serde_json::Value>().await.ok()
                    .and_then(|json| json.get("data").and_then(|d| d.get("id")).and_then(|v| v.as_i64()))
            }
            _ => None,
        };

        if let Some(id) = backtest_id {
            let mc_url = format!("{}/api/backtest/results/{}/monte-carlo?simulations={}", self.api_base, id, sims);
            match self.api_get(&mc_url).await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<serde_json::Value>().await {
                        Ok(json) => {
                            let data = json.get("data").unwrap_or(&json);
                            let embed = embeds::build_montecarlo_embed(&symbol, data);
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
        } else {
            let _ = command.edit_response(&ctx.http,
                EditInteractionResponse::new().content("No backtest results found. Run a backtest first.")).await;
        }
    }

    async fn handle_advanced_strategies(&self, ctx: &Context, command: &CommandInteraction) {
        let _ = command.defer(&ctx.http).await;

        let url = format!("{}/api/strategies/health", self.api_base);
        match self.api_get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let data = json.get("data").unwrap_or(&json);
                        let embed = embeds::build_strategies_embed(data);
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
