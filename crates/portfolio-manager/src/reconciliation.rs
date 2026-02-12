use crate::db::PortfolioDb;
use crate::models::*;
use crate::portfolio::PortfolioManager;
use crate::trades::TradeLogger;
use anyhow::Result;
use rust_decimal::prelude::*;

pub struct Reconciler;

impl Reconciler {
    /// Reconcile local positions against broker positions.
    pub async fn reconcile(
        pm: &PortfolioManager,
        broker: &[BrokerPosition],
        auto_resolve: bool,
    ) -> Result<ReconciliationResult> {
        let local_positions = pm.get_all_positions().await?;
        let mut discrepancies = Vec::new();
        let mut matches = 0usize;
        let mut auto_resolved = 0usize;

        let local_map: std::collections::HashMap<String, &Position> = local_positions
            .iter()
            .map(|p| (p.symbol.clone(), p))
            .collect();

        let broker_map: std::collections::HashMap<String, &BrokerPosition> = broker
            .iter()
            .map(|p| (p.symbol.clone(), p))
            .collect();

        // Check all broker positions
        for bp in broker {
            match local_map.get(&bp.symbol) {
                Some(lp) => {
                    let local_shares = lp.shares.to_f64().unwrap_or(0.0);
                    let broker_shares = bp.shares.to_f64().unwrap_or(0.0);
                    let local_price = lp.entry_price.to_f64().unwrap_or(0.0);
                    let broker_price = bp.avg_entry_price.to_f64().unwrap_or(0.0);

                    let share_diff = (local_shares - broker_shares).abs();
                    let price_diff = (local_price - broker_price).abs();

                    if share_diff > 0.01 {
                        discrepancies.push(Discrepancy {
                            symbol: bp.symbol.clone(),
                            discrepancy_type: "shares_mismatch".to_string(),
                            local_shares: Some(local_shares),
                            broker_shares: Some(broker_shares),
                            local_price: Some(local_price),
                            broker_price: Some(broker_price),
                            resolved: false,
                            resolution: None,
                        });
                    } else if price_diff > 0.01 && auto_resolve {
                        // Auto-resolve price updates
                        if let Some(id) = lp.id {
                            let updated = Position {
                                id: lp.id,
                                symbol: lp.symbol.clone(),
                                shares: lp.shares,
                                entry_price: bp.avg_entry_price,
                                entry_date: lp.entry_date.clone(),
                                notes: lp.notes.clone(),
                                created_at: lp.created_at.clone(),
                            };
                            let _ = pm.update_position(id, updated).await;
                            auto_resolved += 1;
                        }
                        discrepancies.push(Discrepancy {
                            symbol: bp.symbol.clone(),
                            discrepancy_type: "price_drift".to_string(),
                            local_shares: Some(local_shares),
                            broker_shares: Some(broker_shares),
                            local_price: Some(local_price),
                            broker_price: Some(broker_price),
                            resolved: true,
                            resolution: Some("auto_updated_price".to_string()),
                        });
                    } else {
                        matches += 1;
                    }
                }
                None => {
                    discrepancies.push(Discrepancy {
                        symbol: bp.symbol.clone(),
                        discrepancy_type: "missing_local".to_string(),
                        local_shares: None,
                        broker_shares: Some(bp.shares.to_f64().unwrap_or(0.0)),
                        local_price: None,
                        broker_price: Some(bp.avg_entry_price.to_f64().unwrap_or(0.0)),
                        resolved: false,
                        resolution: None,
                    });
                }
            }
        }

        // Check local positions missing from broker
        for lp in &local_positions {
            if !broker_map.contains_key(&lp.symbol) {
                discrepancies.push(Discrepancy {
                    symbol: lp.symbol.clone(),
                    discrepancy_type: "missing_broker".to_string(),
                    local_shares: Some(lp.shares.to_f64().unwrap_or(0.0)),
                    broker_shares: None,
                    local_price: Some(lp.entry_price.to_f64().unwrap_or(0.0)),
                    broker_price: None,
                    resolved: false,
                    resolution: None,
                });
            }
        }

        let total = matches + discrepancies.len();

        Ok(ReconciliationResult {
            reconciliation_date: chrono::Utc::now().to_rfc3339(),
            total_positions: total,
            matches,
            discrepancies,
            auto_resolved,
        })
    }

    /// Save reconciliation result to the log table.
    pub async fn save_log(db: &PortfolioDb, result: &ReconciliationResult) -> Result<i64> {
        let details = serde_json::to_string(&result.discrepancies)?;
        let (id,): (i64,) = sqlx::query_as(
            r#"
            INSERT INTO reconciliation_log
            (reconciliation_date, total_positions, matches, discrepancies, auto_resolved, details_json)
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(&result.reconciliation_date)
        .bind(result.total_positions as i32)
        .bind(result.matches as i32)
        .bind(result.discrepancies.len() as i32)
        .bind(result.auto_resolved as i32)
        .bind(&details)
        .fetch_one(db.pool())
        .await?;

        Ok(id)
    }

    /// Get reconciliation log entries.
    pub async fn get_log(db: &PortfolioDb, limit: i64) -> Result<Vec<ReconciliationLogEntry>> {
        let entries = sqlx::query_as::<_, ReconciliationLogEntry>(
            "SELECT * FROM reconciliation_log ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(db.pool())
        .await?;

        Ok(entries)
    }

    /// Parse CSV trade data.
    /// Expected columns: symbol, action, shares, price, date, commission (optional), notes (optional)
    pub fn parse_csv_trades(csv_data: &str) -> Result<Vec<CsvTradeRow>> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(csv_data.as_bytes());

        let mut rows = Vec::new();
        for result in reader.records() {
            let record = result?;
            let symbol = record.get(0).unwrap_or("").trim().to_uppercase();
            let action = record.get(1).unwrap_or("").trim().to_lowercase();
            let shares: f64 = record
                .get(2)
                .unwrap_or("0")
                .trim()
                .parse()
                .unwrap_or(0.0);
            let price: f64 = record
                .get(3)
                .unwrap_or("0")
                .trim()
                .parse()
                .unwrap_or(0.0);
            let date = record.get(4).unwrap_or("").trim().to_string();
            let commission: Option<f64> = record
                .get(5)
                .and_then(|s| s.trim().parse().ok());
            let notes: Option<String> = record
                .get(6)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            if symbol.is_empty() || date.is_empty() || shares <= 0.0 {
                continue;
            }

            rows.push(CsvTradeRow {
                symbol,
                action,
                shares,
                price,
                date,
                commission,
                notes,
            });
        }

        Ok(rows)
    }

    /// Import CSV trades into the trade logger.
    pub async fn import_csv_trades(
        tl: &TradeLogger,
        rows: &[CsvTradeRow],
    ) -> Result<ImportResult> {
        let mut imported = 0;
        let mut skipped = 0;
        let mut errors = Vec::new();

        for row in rows {
            if !matches!(row.action.as_str(), "buy" | "sell") {
                errors.push(format!(
                    "Invalid action '{}' for {} on {}",
                    row.action, row.symbol, row.date
                ));
                skipped += 1;
                continue;
            }

            let trade = TradeInput {
                symbol: row.symbol.clone(),
                action: row.action.clone(),
                shares: Decimal::from_f64(row.shares).unwrap_or_default(),
                price: Decimal::from_f64(row.price).unwrap_or_default(),
                trade_date: row.date.clone(),
                commission: row.commission.and_then(Decimal::from_f64),
                notes: row.notes.clone(),
                alert_id: None,
                analysis_id: None,
            };

            match tl.log_trade(trade).await {
                Ok(_) => imported += 1,
                Err(e) => {
                    errors.push(format!("{} on {}: {}", row.symbol, row.date, e));
                    skipped += 1;
                }
            }
        }

        Ok(ImportResult {
            imported,
            skipped,
            errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_csv() {
        let csv = "symbol,action,shares,price,date,commission,notes\n\
                   AAPL,buy,10,150.00,2025-01-01,1.00,Test\n\
                   MSFT,sell,5,300.00,2025-01-15,,\n\
                   ,buy,10,100.00,2025-01-01,,\n"; // Empty symbol — should be skipped

        let rows = Reconciler::parse_csv_trades(csv).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].symbol, "AAPL");
        assert_eq!(rows[0].shares, 10.0);
        assert_eq!(rows[1].symbol, "MSFT");
    }

    #[test]
    fn test_parse_csv_empty() {
        let csv = "symbol,action,shares,price,date\n";
        let rows = Reconciler::parse_csv_trades(csv).unwrap();
        assert_eq!(rows.len(), 0);
    }
}
