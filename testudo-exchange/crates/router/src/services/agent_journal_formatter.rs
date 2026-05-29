//! AGENT-03: LLM markdown formatter — transforms AgentSummary into
//! markdown optimized for LLM context windows with citation tokens,
//! per-setup tables, and auto-generated actionable insights.
//!
//! Pure function — no allocations on the hot path beyond building the
//! final String. All formatting is deterministic given the same summary.

// @anchor exchange:router:agent_journal_formatter
// @tags api

use rust_decimal::Decimal;

use crate::models::agent_journal::{AgentSummary, SetupBreakdown, TradeCitation};

/// Format an `AgentSummary` as LLM-optimized markdown.
///
/// Produces sections matching the spec template:
/// - Overall Performance table
/// - By Setup Tag table
/// - Top Performers list with `[T-xxxxxxxx]` citation tokens
/// - Actionable Insights with auto-generated observations
///
/// Designed to fit comfortably in a ~1K token context window.
pub fn format_summary_llm(summary: &AgentSummary) -> String {
    let mut out = String::with_capacity(2048);

    // ── Header ───────────────────────────────────────────────────────
    let symbols: Vec<&str> = summary
        .top_trades
        .iter()
        .map(|t| t.symbol.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let symbol_str = if symbols.is_empty() {
        "All".to_string()
    } else if symbols.len() <= 3 {
        symbols.join(" + ")
    } else {
        format!("{} symbols", symbols.len())
    };

    out.push_str(&format!(
        "## Journal Summary: {} ({})\n\n",
        symbol_str, summary.timeframe.label
    ));

    // ── Overall Performance ──────────────────────────────────────────
    let o = &summary.overall;
    out.push_str("### Overall Performance\n");
    out.push_str(&format!("- Total trades: {}\n", o.trade_count));
    out.push_str(&format!("- Win rate: {:.1}%\n", o.win_rate));
    out.push_str(&format!("- Avg R-multiple: {:.2}\n", o.avg_r_multiple));
    out.push_str(&format!("- Total P&L: ${:.2}\n", o.total_pnl));
    out.push_str(&format!("- Max drawdown: ${:.2}\n", o.max_drawdown));
    out.push_str(&format!("- Profit factor: {:.2}\n", o.profit_factor));
    if let Some(sharpe) = o.sharpe_ratio {
        out.push_str(&format!("- Sharpe ratio: {:.2}\n", sharpe));
    }
    if let Some(hours) = o.avg_hold_hours {
        out.push_str(&format!("- Avg hold time: {:.1}h\n", hours));
    }
    out.push('\n');

    // ── By Setup Tag ─────────────────────────────────────────────────
    if !summary.by_setup.is_empty() {
        out.push_str("### By Setup Tag\n\n");
        out.push_str("| Setup | Trades | Win Rate | Avg R | P&L |\n");
        out.push_str("|---|---|---|---|---|\n");
        for s in &summary.by_setup {
            out.push_str(&format!(
                "| {} | {} | {:.1}% | {:.2} | ${:.2} |\n",
                s.setup,
                s.trade_count,
                s.win_rate,
                s.avg_r_multiple,
                s.total_pnl
            ));
        }
        out.push('\n');
    }

    // ── Top Performers ───────────────────────────────────────────────
    if !summary.top_trades.is_empty() {
        out.push_str("### Top Performers\n\n");
        for t in &summary.top_trades {
            let r_display = t
                .r_multiple
                .map(|r| format!("{:.1}R", r))
                .unwrap_or_else(|| format!("${:.2}", t.pnl));
            let setup_display = t
                .setup_tag
                .as_deref()
                .unwrap_or("untagged");
            let date_display = t.opened_at.format("%Y-%m-%d");
            out.push_str(&format!(
                "- [T-{}] {} {} — {}, {}, opened {}\n",
                t.short_id, t.symbol, t.side, setup_display, r_display, date_display
            ));
        }
        out.push('\n');
    }

    // ── Actionable Insights ──────────────────────────────────────────
    let insights = generate_insights(summary);
    if !insights.is_empty() {
        out.push_str("### Actionable Insights\n\n");
        for insight in &insights {
            out.push_str(&format!("- **{}**: {}\n", insight.0, insight.1));
        }
        out.push('\n');
    }

    out
}

/// Auto-generate observations from the summary data.
/// Each entry is (label, description). Deterministic — no randomness.
fn generate_insights(summary: &AgentSummary) -> Vec<(&'static str, String)> {
    let mut insights = Vec::new();

    // Best setup by win rate (min 3 trades for signal)
    if let Some(best) = summary
        .by_setup
        .iter()
        .filter(|s| s.trade_count >= 3)
        .max_by(|a, b| {
            a.win_rate
                .partial_cmp(&b.win_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    {
        if best.win_rate > Decimal::from(55) {
            insights.push((
                "Strongest setup",
                format!(
                    "{} shows {:.1}% win rate with {:.2} avg R over {} trades. \
                     Consider increasing allocation.",
                    best.setup, best.win_rate, best.avg_r_multiple, best.trade_count,
                ),
            ));
        }
    }

    // Worst setup by win rate (min 3 trades)
    if let Some(worst) = summary
        .by_setup
        .iter()
        .filter(|s| s.trade_count >= 3)
        .min_by(|a, b| {
            a.win_rate
                .partial_cmp(&b.win_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    {
        if worst.win_rate < Decimal::from(45) {
            insights.push((
                "Underperforming setup",
                format!(
                    "{} has {:.1}% win rate over {} trades. \
                     Review entry criteria or reduce position size.",
                    worst.setup, worst.win_rate, worst.trade_count,
                ),
            ));
        }
    }

    // Overall win rate signal
    if summary.overall.trade_count >= 10 && summary.overall.win_rate < Decimal::from(40) {
        insights.push((
            "Low overall win rate",
            format!(
                "{:.1}% win rate over {} trades. \
                 Verify strategy alignment and position sizing.",
                summary.overall.win_rate, summary.overall.trade_count,
            ),
        ));
    }

    // Negative total P&L
    if summary.overall.total_pnl < Decimal::ZERO {
        insights.push((
            "Negative P&L",
            format!(
                "Total P&L is ${:.2} over {} trades. \
                 Consider reducing trade frequency until edge is confirmed.",
                summary.overall.total_pnl, summary.overall.trade_count,
            ),
        ));
    }

    insights
}

// ── Unit tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::agent_journal::{
        AgentSummary, EquityPoint, OverallStats, SetupBreakdown, TimeframeInfo, TradeCitation,
    };
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn make_summary() -> AgentSummary {
        let trade_id = Uuid::parse_str("a3f2b1c4-1111-2222-3333-444455556666").unwrap();
        let trade_id2 = Uuid::parse_str("b7c1d2e3-1111-2222-3333-444455556666").unwrap();
        AgentSummary {
            timeframe: TimeframeInfo {
                label: "Last 90 Days".into(),
                from: None,
                to: None,
            },
            overall: OverallStats {
                trade_count: 112,
                win_rate: dec!(54.5),
                avg_r_multiple: dec!(1.72),
                total_pnl: dec!(8420.50),
                max_drawdown: dec!(-1890.00),
                profit_factor: dec!(1.83),
                sharpe_ratio: Some(dec!(1.21)),
                avg_hold_hours: Some(dec!(4.5)),
            },
            by_setup: vec![
                SetupBreakdown {
                    setup: "breakout".into(),
                    trade_count: 28,
                    win_rate: dec!(60.7),
                    avg_r_multiple: dec!(2.1),
                    total_pnl: dec!(3240.00),
                },
                SetupBreakdown {
                    setup: "support_bounce".into(),
                    trade_count: 34,
                    win_rate: dec!(55.9),
                    avg_r_multiple: dec!(1.8),
                    total_pnl: dec!(2850.00),
                },
                SetupBreakdown {
                    setup: "trend_follow".into(),
                    trade_count: 22,
                    win_rate: dec!(40.9),
                    avg_r_multiple: dec!(0.9),
                    total_pnl: dec!(-920.00),
                },
                SetupBreakdown {
                    setup: "reversal".into(),
                    trade_count: 28,
                    win_rate: dec!(53.6),
                    avg_r_multiple: dec!(1.5),
                    total_pnl: dec!(3250.00),
                },
            ],
            top_trades: vec![
                TradeCitation {
                    id: trade_id,
                    short_id: "a3f2b1c4".into(),
                    symbol: "BTC_USDT".into(),
                    side: "LONG".into(),
                    opened_at: NaiveDate::from_ymd_opt(2026, 3, 15)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc(),
                    pnl: dec!(4200.00),
                    r_multiple: Some(dec!(4.2)),
                    setup_tag: Some("breakout".into()),
                },
                TradeCitation {
                    id: trade_id2,
                    short_id: "b7c1d2e3".into(),
                    symbol: "ETH_USDT".into(),
                    side: "SHORT".into(),
                    opened_at: NaiveDate::from_ymd_opt(2026, 4, 2)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc(),
                    pnl: dec!(3100.00),
                    r_multiple: Some(dec!(3.1)),
                    setup_tag: Some("support_bounce".into()),
                },
            ],
            equity: vec![EquityPoint {
                date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
                cumulative_pnl: dec!(1000.00),
                equity: None,
            }],
        }
    }

    #[test]
    fn test_format_summary_llm_contains_header() {
        let summary = make_summary();
        let md = format_summary_llm(&summary);
        assert!(md.contains("## Journal Summary:"));
        assert!(md.contains("Last 90 Days"));
    }

    #[test]
    fn test_format_summary_llm_contains_overall_performance() {
        let summary = make_summary();
        let md = format_summary_llm(&summary);
        assert!(md.contains("### Overall Performance"));
        assert!(md.contains("Total trades: 112"));
        assert!(md.contains("Win rate: 54.5%"));
        assert!(md.contains("Sharpe ratio: 1.21"));
    }

    #[test]
    fn test_format_summary_llm_contains_setup_table() {
        let summary = make_summary();
        let md = format_summary_llm(&summary);
        assert!(md.contains("### By Setup Tag"));
        assert!(md.contains("| Setup | Trades | Win Rate | Avg R | P&L |"));
        assert!(md.contains("breakout"));
        assert!(md.contains("trend_follow"));
    }

    #[test]
    fn test_format_summary_llm_contains_citation_tokens() {
        let summary = make_summary();
        let md = format_summary_llm(&summary);
        assert!(md.contains("[T-a3f2b1c4]"));
        assert!(md.contains("[T-b7c1d2e3]"));
    }

    #[test]
    fn test_format_summary_llm_contains_top_performers() {
        let summary = make_summary();
        let md = format_summary_llm(&summary);
        assert!(md.contains("### Top Performers"));
        assert!(md.contains("BTC_USDT LONG"));
        assert!(md.contains("ETH_USDT SHORT"));
    }

    #[test]
    fn test_format_summary_llm_contains_actionable_insights() {
        let summary = make_summary();
        let md = format_summary_llm(&summary);
        assert!(md.contains("### Actionable Insights"));
        // breakout should appear as strongest
        assert!(md.contains("Strongest setup"));
        // trend_follow should appear as underperforming (40.9% < 45%)
        assert!(md.contains("Underperforming setup"));
    }

    #[test]
    fn test_empty_summary_no_panics() {
        let summary = AgentSummary {
            timeframe: TimeframeInfo {
                label: "No Data".into(),
                from: None,
                to: None,
            },
            overall: OverallStats {
                trade_count: 0,
                win_rate: Decimal::ZERO,
                avg_r_multiple: Decimal::ZERO,
                total_pnl: Decimal::ZERO,
                max_drawdown: Decimal::ZERO,
                profit_factor: Decimal::ZERO,
                sharpe_ratio: None,
                avg_hold_hours: None,
            },
            by_setup: vec![],
            top_trades: vec![],
            equity: vec![],
        };
        let md = format_summary_llm(&summary);
        // Should not panic and should produce valid markdown
        assert!(md.contains("## Journal Summary"));
        assert!(!md.contains("### By Setup Tag")); // no setups
        assert!(!md.contains("### Top Performers")); // no trades
    }

    #[test]
    fn test_negative_pnl_generates_warning() {
        let mut summary = make_summary();
        summary.overall.total_pnl = dec!(-500.00);
        summary.overall.trade_count = 50;
        let md = format_summary_llm(&summary);
        assert!(md.contains("Negative P&L"));
    }

    #[test]
    fn test_low_win_rate_generates_warning() {
        let mut summary = make_summary();
        summary.overall.win_rate = dec!(35.0);
        summary.overall.trade_count = 20;
        let md = format_summary_llm(&summary);
        assert!(md.contains("Low overall win rate"));
    }
}
