mod charts;

use analysis_core::{Bar, SignalStrength, Timeframe};
use analysis_orchestrator::AnalysisOrchestrator;
use serenity::{
    async_trait,
    builder::{CreateAttachment, CreateMessage},
    model::{channel::Message, gateway::Ready},
    prelude::*,
};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const API_BASE_URL: &str = "http://localhost:3000";

struct Handler {
    orchestrator: Arc<AnalysisOrchestrator>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore bot messages
        if msg.author.bot {
            return;
        }

        // Check if message starts with !iq
        if !msg.content.starts_with("!iq") {
            return;
        }

        let parts: Vec<&str> = msg.content.split_whitespace().collect();

        if parts.len() < 2 {
            let _ = msg
                .channel_id
                .say(&ctx.http, "Usage: `!iq <command> [args]`\n\nCommands:\n- `analyze <SYMBOL>` - Get comprehensive stock analysis with chart\n- `chart <SYMBOL>` - Get chart only\n- `help` - Show this help message")
                .await;
            return;
        }

        match parts[1] {
            "analyze" => {
                if parts.len() < 3 {
                    let _ = msg
                        .channel_id
                        .say(&ctx.http, "Usage: `!iq analyze <SYMBOL>`")
                        .await;
                    return;
                }

                let symbol = parts[2].to_uppercase();

                // Send typing indicator
                let _ = msg.channel_id.broadcast_typing(&ctx.http).await;

                match self.orchestrator.analyze(&symbol, Timeframe::Day1, 365).await {
                    Ok(analysis) => {
                        // Generate text analysis
                        let response = format_analysis(&symbol, &analysis);

                        // Try to generate and send chart
                        match fetch_and_generate_chart(&symbol, &analysis.overall_signal).await {
                            Ok(chart_path) => {
                                // Send chart with analysis
                                let attachment = CreateAttachment::path(&chart_path).await;

                                if let Ok(attachment) = attachment {
                                    let builder = CreateMessage::default()
                                        .content(response)
                                        .add_file(attachment);

                                    let _ = msg.channel_id.send_message(&ctx.http, builder).await;

                                    // Clean up temp file
                                    let _ = std::fs::remove_file(chart_path);
                                } else {
                                    // Send text only if chart upload fails
                                    let _ = msg.channel_id.say(&ctx.http, response).await;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to generate chart: {}", e);
                                // Send text analysis anyway
                                let _ = msg.channel_id.say(&ctx.http, response).await;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = msg
                            .channel_id
                            .say(&ctx.http, format!("❌ Error analyzing {}: {}", symbol, e))
                            .await;
                    }
                }
            }
            "chart" => {
                if parts.len() < 3 {
                    let _ = msg
                        .channel_id
                        .say(&ctx.http, "Usage: `!iq chart <SYMBOL>`")
                        .await;
                    return;
                }

                let symbol = parts[2].to_uppercase();

                // Send typing indicator
                let _ = msg.channel_id.broadcast_typing(&ctx.http).await;

                // Get signal for chart coloring
                match self.orchestrator.analyze(&symbol, Timeframe::Day1, 365).await {
                    Ok(analysis) => {
                        match fetch_and_generate_chart(&symbol, &analysis.overall_signal).await {
                            Ok(chart_path) => {
                                let attachment = CreateAttachment::path(&chart_path).await;

                                if let Ok(attachment) = attachment {
                                    let caption = format!(
                                        "**{}** - {:?} ({:.0}% confidence)",
                                        symbol,
                                        analysis.overall_signal,
                                        analysis.overall_confidence * 100.0
                                    );

                                    let builder = CreateMessage::default()
                                        .content(caption)
                                        .add_file(attachment);

                                    let _ = msg.channel_id.send_message(&ctx.http, builder).await;

                                    // Clean up temp file
                                    let _ = std::fs::remove_file(chart_path);
                                } else {
                                    let _ = msg
                                        .channel_id
                                        .say(&ctx.http, "❌ Failed to upload chart")
                                        .await;
                                }
                            }
                            Err(e) => {
                                let _ = msg
                                    .channel_id
                                    .say(&ctx.http, format!("❌ Error generating chart: {}", e))
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = msg
                            .channel_id
                            .say(&ctx.http, format!("❌ Error: {}", e))
                            .await;
                    }
                }
            }
            "help" => {
                let help_text = r#"
**InvestIQ Bot Commands**

`!iq analyze <SYMBOL>` - Get comprehensive stock analysis with chart
Example: `!iq analyze AAPL`

`!iq chart <SYMBOL>` - Get chart only (faster)
Example: `!iq chart TSLA`

`!iq help` - Show this help message

The bot combines technical, fundamental, quantitative, and sentiment analysis to provide stock recommendations.

**Chart Features:**
📊 Candlestick price chart with SMA
📈 RSI indicator with overbought/oversold zones
📉 MACD with signal line and histogram
                "#;
                let _ = msg.channel_id.say(&ctx.http, help_text).await;
            }
            _ => {
                let _ = msg
                    .channel_id
                    .say(
                        &ctx.http,
                        format!("Unknown command: `{}`\nUse `!iq help` for available commands", parts[1]),
                    )
                    .await;
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        tracing::info!("{} is connected and ready!", ready.user.name);
    }
}

async fn fetch_and_generate_chart(
    symbol: &str,
    signal: &SignalStrength,
) -> Result<std::path::PathBuf, anyhow::Error> {
    // Fetch bars from API
    let url = format!("{}/api/bars/{}?timeframe=1d&days=90", API_BASE_URL, symbol);

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to fetch data from API"));
    }

    let json: serde_json::Value = response.json().await?;

    if !json.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Err(anyhow::anyhow!("API returned error"));
    }

    let bars_data = json.get("data")
        .ok_or_else(|| anyhow::anyhow!("No data in response"))?;

    let bars: Vec<Bar> = serde_json::from_value(bars_data.clone())?;

    if bars.is_empty() {
        return Err(anyhow::anyhow!("No bar data available"));
    }

    // Generate chart
    charts::generate_chart(symbol, &bars, signal).await
}

fn format_analysis(symbol: &str, analysis: &analysis_core::UnifiedAnalysis) -> String {
    let signal_emoji = match analysis.overall_signal {
        SignalStrength::StrongBuy => "🚀",
        SignalStrength::Buy => "📈",
        SignalStrength::WeakBuy => "↗️",
        SignalStrength::Neutral => "➡️",
        SignalStrength::WeakSell => "↘️",
        SignalStrength::Sell => "📉",
        SignalStrength::StrongSell => "⚠️",
    };

    let mut response = format!(
        "**{} Analysis for {}**\n\n",
        signal_emoji, symbol
    );

    response.push_str(&format!(
        "**Overall Signal:** {:?}\n",
        analysis.overall_signal
    ));
    response.push_str(&format!(
        "**Recommendation:** {}\n\n",
        analysis.recommendation
    ));

    // Technical Analysis
    if let Some(tech) = &analysis.technical {
        response.push_str(&format!(
            "📊 **Technical:** {:?} ({:.0}% confidence)\n",
            tech.signal,
            tech.confidence * 100.0
        ));
        response.push_str(&format!("   _{}_\n", tech.reason));
    }

    // Fundamental Analysis
    if let Some(fund) = &analysis.fundamental {
        response.push_str(&format!(
            "💼 **Fundamental:** {:?} ({:.0}% confidence)\n",
            fund.signal,
            fund.confidence * 100.0
        ));
        response.push_str(&format!("   _{}_\n", fund.reason));
    }

    // Quantitative Analysis
    if let Some(quant) = &analysis.quantitative {
        response.push_str(&format!(
            "🔢 **Quantitative:** {:?} ({:.0}% confidence)\n",
            quant.signal,
            quant.confidence * 100.0
        ));
        if let Some(sharpe) = quant.metrics.get("sharpe_ratio") {
            response.push_str(&format!("   _Sharpe Ratio: {:.2}_\n", sharpe));
        }
    }

    // Sentiment Analysis
    if let Some(sent) = &analysis.sentiment {
        response.push_str(&format!(
            "📰 **Sentiment:** {:?} ({:.0}% confidence)\n",
            sent.signal,
            sent.confidence * 100.0
        ));
        response.push_str(&format!("   _{}_\n", sent.reason));
    }

    response.push_str(&format!(
        "\n_Analysis performed at {}_",
        analysis.timestamp.format("%Y-%m-%d %H:%M UTC")
    ));

    response
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "discord_bot=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get tokens from environment
    let discord_token = std::env::var("DISCORD_BOT_TOKEN")
        .expect("DISCORD_BOT_TOKEN must be set in environment");
    let polygon_api_key = std::env::var("POLYGON_API_KEY")
        .expect("POLYGON_API_KEY must be set in environment");

    // Create orchestrator
    let orchestrator = Arc::new(AnalysisOrchestrator::new(polygon_api_key));

    // Create Discord client
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&discord_token, intents)
        .event_handler(Handler { orchestrator })
        .await?;

    tracing::info!("Discord bot starting...");
    tracing::info!("📊 Chart generation enabled!");

    // Start the bot
    client.start().await?;

    Ok(())
}
