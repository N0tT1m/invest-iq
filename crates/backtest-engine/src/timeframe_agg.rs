use rust_decimal::Decimal;

use crate::models::HistoricalBar;

/// Aggregate daily bars into weekly bars.
///
/// Groups bars by ISO week and produces one bar per week with:
/// - open = first bar's open
/// - high = max high in the week
/// - low = min low in the week
/// - close = last bar's close
/// - volume = sum of all volumes
pub fn aggregate_to_weekly(daily_bars: &[HistoricalBar]) -> Vec<HistoricalBar> {
    if daily_bars.is_empty() {
        return Vec::new();
    }

    let mut weekly: Vec<HistoricalBar> = Vec::new();
    let mut current_week: Option<(u32, Vec<&HistoricalBar>)> = None;

    for bar in daily_bars {
        let week_num = iso_week_number(&bar.date);

        match &mut current_week {
            Some((w, bars)) if *w == week_num => {
                bars.push(bar);
            }
            _ => {
                // Flush previous week
                if let Some((_, bars)) = current_week.take() {
                    if let Some(weekly_bar) = make_weekly_bar(&bars) {
                        weekly.push(weekly_bar);
                    }
                }
                current_week = Some((week_num, vec![bar]));
            }
        }
    }

    // Flush last week
    if let Some((_, bars)) = current_week {
        if let Some(weekly_bar) = make_weekly_bar(&bars) {
            weekly.push(weekly_bar);
        }
    }

    weekly
}

fn make_weekly_bar(bars: &[&HistoricalBar]) -> Option<HistoricalBar> {
    if bars.is_empty() {
        return None;
    }

    let open = bars.first().unwrap().open;
    let close = bars.last().unwrap().close;
    let high = bars
        .iter()
        .map(|b| b.high)
        .fold(Decimal::MIN, |a, b| a.max(b));
    let low = bars
        .iter()
        .map(|b| b.low)
        .fold(Decimal::MAX, |a, b| a.min(b));
    let volume: f64 = bars.iter().map(|b| b.volume).sum();
    let date = bars.last().unwrap().date.clone(); // Use Friday's date

    Some(HistoricalBar {
        date,
        open,
        high,
        low,
        close,
        volume,
    })
}

/// Get the ISO week number from a "YYYY-MM-DD" date string.
/// Returns year * 100 + week to handle year boundaries.
fn iso_week_number(date_str: &str) -> u32 {
    use chrono::Datelike;
    use chrono::NaiveDate;

    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map(|d| d.iso_week().year() as u32 * 100 + d.iso_week().week())
        .unwrap_or(0)
}
