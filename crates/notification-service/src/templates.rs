use crate::{Alert, AlertType};

pub struct EmailTemplate;

impl EmailTemplate {
    pub fn render(alert: &Alert) -> String {
        let body_content = match &alert.alert_type {
            AlertType::TradeExecuted {
                symbol,
                action,
                shares,
                price,
                confidence,
            } => {
                let action_color = if action == "buy" {
                    "#22c55e"
                } else {
                    "#ef4444"
                };
                let action_label = action.to_uppercase();
                let conf_html = confidence
                    .map(|c| format!(r#"<tr><td style="padding:8px 12px;color:#94a3b8;">Confidence</td><td style="padding:8px 12px;font-weight:600;">{:.0}%</td></tr>"#, c * 100.0))
                    .unwrap_or_default();
                format!(
                    r#"<div style="background:{action_color};color:#fff;padding:12px 20px;border-radius:8px 8px 0 0;font-size:18px;font-weight:700;">{action_label} {symbol}</div>
<table style="width:100%;border-collapse:collapse;">
  <tr><td style="padding:8px 12px;color:#94a3b8;">Symbol</td><td style="padding:8px 12px;font-weight:600;">{symbol}</td></tr>
  <tr style="background:#f8fafc;"><td style="padding:8px 12px;color:#94a3b8;">Action</td><td style="padding:8px 12px;font-weight:600;color:{action_color};">{action_label}</td></tr>
  <tr><td style="padding:8px 12px;color:#94a3b8;">Shares</td><td style="padding:8px 12px;font-weight:600;">{shares:.2}</td></tr>
  <tr style="background:#f8fafc;"><td style="padding:8px 12px;color:#94a3b8;">Price</td><td style="padding:8px 12px;font-weight:600;">${price:.2}</td></tr>
  {conf_html}
</table>"#
                )
            }
            AlertType::CircuitBreakerTripped { reason } => {
                format!(
                    r#"<div style="background:#ef4444;color:#fff;padding:12px 20px;border-radius:8px 8px 0 0;font-size:18px;font-weight:700;">CIRCUIT BREAKER TRIPPED</div>
<div style="padding:16px 20px;">
  <p style="color:#ef4444;font-weight:600;font-size:16px;margin:0 0 8px;">Trading has been halted</p>
  <p style="color:#334155;margin:0;">{reason}</p>
</div>"#
                )
            }
            AlertType::StopLossHit {
                symbol,
                entry_price,
                stop_price,
                loss_percent,
            } => {
                format!(
                    r#"<div style="background:#f97316;color:#fff;padding:12px 20px;border-radius:8px 8px 0 0;font-size:18px;font-weight:700;">STOP-LOSS HIT &mdash; {symbol}</div>
<table style="width:100%;border-collapse:collapse;">
  <tr><td style="padding:8px 12px;color:#94a3b8;">Symbol</td><td style="padding:8px 12px;font-weight:600;">{symbol}</td></tr>
  <tr style="background:#f8fafc;"><td style="padding:8px 12px;color:#94a3b8;">Entry Price</td><td style="padding:8px 12px;font-weight:600;">${entry_price:.2}</td></tr>
  <tr><td style="padding:8px 12px;color:#94a3b8;">Stop Price</td><td style="padding:8px 12px;font-weight:600;">${stop_price:.2}</td></tr>
  <tr style="background:#f8fafc;"><td style="padding:8px 12px;color:#94a3b8;">Loss</td><td style="padding:8px 12px;font-weight:600;color:#ef4444;">-{loss_percent:.2}%</td></tr>
</table>"#
                )
            }
            AlertType::AgentTradeProposal {
                symbol,
                action,
                confidence,
                reason,
            } => {
                let action_color = if action == "buy" {
                    "#22c55e"
                } else {
                    "#ef4444"
                };
                format!(
                    r#"<div style="background:#3b82f6;color:#fff;padding:12px 20px;border-radius:8px 8px 0 0;font-size:18px;font-weight:700;">Agent Trade Proposal &mdash; {symbol}</div>
<table style="width:100%;border-collapse:collapse;">
  <tr><td style="padding:8px 12px;color:#94a3b8;">Symbol</td><td style="padding:8px 12px;font-weight:600;">{symbol}</td></tr>
  <tr style="background:#f8fafc;"><td style="padding:8px 12px;color:#94a3b8;">Action</td><td style="padding:8px 12px;font-weight:600;color:{action_color};">{}</td></tr>
  <tr><td style="padding:8px 12px;color:#94a3b8;">Confidence</td><td style="padding:8px 12px;font-weight:600;">{confidence:.0}%</td></tr>
  <tr style="background:#f8fafc;"><td style="padding:8px 12px;color:#94a3b8;">Reason</td><td style="padding:8px 12px;">{reason}</td></tr>
</table>
<div style="padding:16px 20px;text-align:center;">
  <p style="color:#64748b;margin:0;">Review this proposal in the dashboard to approve or reject.</p>
</div>"#,
                    action.to_uppercase()
                )
            }
            AlertType::DailyReport {
                date,
                pnl,
                trades_count,
                positions_count,
            } => {
                let pnl_color = if *pnl >= 0.0 { "#22c55e" } else { "#ef4444" };
                let pnl_sign = if *pnl >= 0.0 { "+" } else { "" };
                format!(
                    r#"<div style="background:#1e293b;color:#fff;padding:12px 20px;border-radius:8px 8px 0 0;font-size:18px;font-weight:700;">Daily Report &mdash; {date}</div>
<table style="width:100%;border-collapse:collapse;">
  <tr><td style="padding:8px 12px;color:#94a3b8;">Date</td><td style="padding:8px 12px;font-weight:600;">{date}</td></tr>
  <tr style="background:#f8fafc;"><td style="padding:8px 12px;color:#94a3b8;">P&amp;L</td><td style="padding:8px 12px;font-weight:600;color:{pnl_color};">{pnl_sign}${pnl:.2}</td></tr>
  <tr><td style="padding:8px 12px;color:#94a3b8;">Trades</td><td style="padding:8px 12px;font-weight:600;">{trades_count}</td></tr>
  <tr style="background:#f8fafc;"><td style="padding:8px 12px;color:#94a3b8;">Open Positions</td><td style="padding:8px 12px;font-weight:600;">{positions_count}</td></tr>
</table>"#
                )
            }
        };

        format!(
            r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="margin:0;padding:0;background:#f1f5f9;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
<table width="100%" cellpadding="0" cellspacing="0" style="background:#f1f5f9;padding:32px 0;">
  <tr><td align="center">
    <table width="600" cellpadding="0" cellspacing="0" style="background:#ffffff;border-radius:8px;overflow:hidden;box-shadow:0 1px 3px rgba(0,0,0,0.1);">
      <tr><td>
        {body_content}
      </td></tr>
      <tr><td style="padding:16px 20px;border-top:1px solid #e2e8f0;">
        <p style="margin:0;color:#94a3b8;font-size:12px;">
          {msg}
          <br>Sent at {ts} UTC
        </p>
      </td></tr>
    </table>
    <p style="color:#94a3b8;font-size:11px;margin-top:16px;">InvestIQ Notification Service</p>
  </td></tr>
</table>
</body>
</html>"#,
            msg = alert.message.replace('<', "&lt;").replace('>', "&gt;"),
            ts = alert.timestamp.format("%Y-%m-%d %H:%M:%S"),
        )
    }
}
