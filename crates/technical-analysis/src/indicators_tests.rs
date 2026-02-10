#[cfg(test)]
mod tests {
    use super::super::indicators::*;
    use analysis_core::Bar;
    use chrono::Utc;

    // Helper function to create sample price data
    fn sample_prices() -> Vec<f64> {
        vec![
            44.34, 44.09, 44.15, 43.61, 44.33, 44.83, 45.10, 45.42, 45.84, 46.08,
            45.89, 46.03, 45.61, 46.28, 46.28, 46.00, 46.03, 46.41, 46.22, 45.64,
        ]
    }

    // Helper function to create sample bars
    fn sample_bars() -> Vec<Bar> {
        let prices = vec![
            (100.0, 102.0, 99.0, 101.0),
            (101.0, 103.0, 100.0, 102.0),
            (102.0, 104.0, 101.0, 103.0),
            (103.0, 105.0, 102.0, 104.0),
            (104.0, 106.0, 103.0, 105.0),
            (105.0, 107.0, 104.0, 106.0),
            (106.0, 108.0, 105.0, 107.0),
            (107.0, 109.0, 106.0, 108.0),
            (108.0, 110.0, 107.0, 109.0),
            (109.0, 111.0, 108.0, 110.0),
            (110.0, 112.0, 109.0, 111.0),
            (111.0, 113.0, 110.0, 112.0),
            (112.0, 114.0, 111.0, 113.0),
            (113.0, 115.0, 112.0, 114.0),
            (114.0, 116.0, 113.0, 115.0),
        ];

        prices
            .into_iter()
            .enumerate()
            .map(|(i, (open, high, low, close))| Bar {
                timestamp: Utc::now() - chrono::Duration::days(15 - i as i64),
                open,
                high,
                low,
                close,
                volume: 1000000.0,
            })
            .collect()
    }

    #[test]
    fn test_sma_basic() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = sma(&data, 3);

        assert_eq!(result.len(), 3);
        assert!((result[0] - 2.0).abs() < 0.001); // (1+2+3)/3 = 2
        assert!((result[1] - 3.0).abs() < 0.001); // (2+3+4)/3 = 3
        assert!((result[2] - 4.0).abs() < 0.001); // (3+4+5)/3 = 4
    }

    #[test]
    fn test_sma_insufficient_data() {
        let data = vec![1.0, 2.0];
        let result = sma(&data, 5);

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_sma_real_prices() {
        let prices = sample_prices();
        let result = sma(&prices, 5);

        assert!(!result.is_empty());
        // First SMA(5) should be average of first 5 prices
        let expected_first = (44.34 + 44.09 + 44.15 + 43.61 + 44.33) / 5.0;
        assert!((result[0] - expected_first).abs() < 0.01);
    }

    #[test]
    fn test_ema_basic() {
        let data = vec![22.0, 24.0, 23.0, 25.0, 26.0];
        let result = ema(&data, 3);

        assert_eq!(result.len(), data.len());
        // EMA should start with SMA
        let first_sma = (22.0 + 24.0 + 23.0) / 3.0;
        assert!((result[0] - first_sma).abs() < 0.01);
    }

    #[test]
    fn test_ema_empty_data() {
        let data: Vec<f64> = vec![];
        let result = ema(&data, 5);

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_ema_increases_with_uptrend() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let result = ema(&data, 3);

        // EMA should generally increase with uptrend
        for i in 1..result.len() {
            assert!(result[i] > result[i - 1]);
        }
    }

    #[test]
    fn test_rsi_basic() {
        let prices = sample_prices();
        let result = rsi(&prices, 14);

        assert!(!result.is_empty());
        // RSI should be between 0 and 100
        for &value in &result {
            assert!(value >= 0.0 && value <= 100.0);
        }
    }

    #[test]
    fn test_rsi_insufficient_data() {
        let data = vec![1.0, 2.0, 3.0];
        let result = rsi(&data, 14);

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_rsi_overbought_oversold() {
        // Create data that should produce extreme RSI
        let mut uptrend = vec![100.0];
        for i in 1..20 {
            uptrend.push(100.0 + i as f64);
        }

        let result = rsi(&uptrend, 14);
        // Strong uptrend should produce high RSI (overbought)
        assert!(result.last().unwrap() > &70.0);
    }

    #[test]
    fn test_macd_basic() {
        let prices = sample_prices();
        let result = macd(&prices, 12, 26, 9);

        assert!(!result.macd_line.is_empty());
        assert!(!result.signal_line.is_empty());
        assert!(!result.histogram.is_empty());
        assert_eq!(result.histogram.len(), result.signal_line.len());
    }

    #[test]
    fn test_macd_histogram() {
        let prices = sample_prices();
        let result = macd(&prices, 12, 26, 9);

        // Histogram should be macd_line - signal_line
        for (i, &hist) in result.histogram.iter().enumerate() {
            let offset = result.macd_line.len() - result.signal_line.len();
            let expected = result.macd_line[i + offset] - result.signal_line[i];
            assert!((hist - expected).abs() < 0.001);
        }
    }

    #[test]
    fn test_bollinger_bands_basic() {
        let prices = sample_prices();
        let result = bollinger_bands(&prices, 20, 2.0);

        assert_eq!(result.upper.len(), result.middle.len());
        assert_eq!(result.middle.len(), result.lower.len());
    }

    #[test]
    fn test_bollinger_bands_ordering() {
        let prices = sample_prices();
        let result = bollinger_bands(&prices, 10, 2.0);

        // Upper band should be above middle, middle above lower
        for i in 0..result.upper.len() {
            assert!(result.upper[i] > result.middle[i]);
            assert!(result.middle[i] > result.lower[i]);
        }
    }

    #[test]
    fn test_bollinger_bands_width() {
        let prices = vec![100.0; 20]; // Constant prices
        let result = bollinger_bands(&prices, 10, 2.0);

        // With constant prices, bands should be very close together
        for i in 0..result.upper.len() {
            let width = result.upper[i] - result.lower[i];
            assert!(width < 1.0); // Very narrow bands
        }
    }

    #[test]
    fn test_atr_basic() {
        let bars = sample_bars();
        let result = atr(&bars, 14);

        assert!(!result.is_empty());
        // ATR should be positive
        for &value in &result {
            assert!(value > 0.0);
        }
    }

    #[test]
    fn test_atr_insufficient_data() {
        let bars = sample_bars()[..5].to_vec();
        let result = atr(&bars, 14);

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_atr_increases_with_volatility() {
        let bars = sample_bars();
        let normal_atr = atr(&bars, 5);

        // Create high volatility bars
        let mut volatile_bars = sample_bars();
        for bar in &mut volatile_bars {
            bar.high += 10.0;
            bar.low -= 10.0;
        }
        let volatile_atr = atr(&volatile_bars, 5);

        // ATR should be higher for more volatile data
        assert!(volatile_atr[0] > normal_atr[0]);
    }

    #[test]
    fn test_obv_basic() {
        let bars = sample_bars();
        let result = obv(&bars);

        assert_eq!(result.len(), bars.len());
    }

    #[test]
    fn test_obv_increases_on_up_days() {
        let mut bars = sample_bars();
        // Make all days up days
        for bar in &mut bars {
            bar.close = bar.open + 1.0;
        }
        let result = obv(&bars);

        // OBV should increase on consecutive up days
        for i in 1..result.len() {
            assert!(result[i] > result[i - 1]);
        }
    }

    #[test]
    fn test_obv_decreases_on_down_days() {
        let mut bars = sample_bars();
        // Make all days down days
        for bar in &mut bars {
            bar.close = bar.open - 1.0;
        }
        let result = obv(&bars);

        // OBV should decrease on consecutive down days
        for i in 1..result.len() {
            assert!(result[i] < result[i - 1]);
        }
    }

    #[test]
    fn test_vwap_basic() {
        let bars = sample_bars();
        let result = vwap(&bars);

        assert!(!result.is_empty());
        // VWAP should be within the price range
        for (i, &value) in result.iter().enumerate() {
            assert!(value >= bars[i].low && value <= bars[i].high);
        }
    }

    #[test]
    fn test_vwap_with_zero_volume() {
        let mut bars = sample_bars();
        bars[0].volume = 0.0;
        let result = vwap(&bars);

        // Should handle zero volume gracefully
        assert!(!result.is_empty());
    }

    #[test]
    fn test_stochastic_basic() {
        let bars = sample_bars();
        let result = stochastic(&bars, 14, 3);

        assert!(!result.k.is_empty());
        assert!(!result.d.is_empty());

        // %K and %D should be between 0 and 100
        for &value in &result.k {
            assert!(value >= 0.0 && value <= 100.0);
        }
        for &value in &result.d {
            assert!(value >= 0.0 && value <= 100.0);
        }
    }

    #[test]
    fn test_stochastic_insufficient_data() {
        let bars = sample_bars()[..5].to_vec();
        let result = stochastic(&bars, 14, 3);

        // Should handle insufficient data gracefully
        assert_eq!(result.k.len(), 0);
    }
}
