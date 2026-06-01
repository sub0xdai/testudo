// @anchor infra:cli:view:banner
// @tags ui

//! Welcome banner — Testudo ASCII art splash screen.
//! Renders the testudo (tortoise) shield-wall formation of Roman legionaries.

use crate::theme::Theme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn render(frame: &mut Frame, theme: &Theme, area: Rect) {
    let accent = Style::default().fg(theme.accent);
    let fg = Style::default().fg(theme.fg);
    let muted = Style::default().fg(theme.muted);
    let dim = Style::default().fg(theme.dim_fg);
    let gold = Style::default().fg(theme.warning); // gold/amber for Roman aesthetic
    let danger = Style::default().fg(theme.danger);

    // Center the banner vertically and horizontally
    let banner_height = 26u16; // lines in the banner
    let banner_width = 62u16; // max line width

    let vertical_pad = area.height.saturating_sub(banner_height) / 2;
    let horizontal_pad = area.width.saturating_sub(banner_width) / 2;

    let centered = Rect {
        x: area.x + horizontal_pad,
        y: area.y + vertical_pad,
        width: banner_width,
        height: banner_height,
    };

    let lines = vec![
        // ═══ top border ═══
        Line::from(Span::styled(
            "╔══════════════════════════════════════════════════════════════╗",
            muted,
        )),
        Line::from(Span::styled(
            "║                                                              ║",
            muted,
        )),
        // TESTUDO in big block letters
        Line::from(vec![
            Span::styled("║  ", muted),
            Span::styled("████████╗", accent),
            Span::styled("███████╗", accent),
            Span::styled("███████╗", accent),
            Span::styled("████████╗", accent),
            Span::styled("██╗   ██╗", accent),
            Span::styled("██████╗ ", accent),
            Span::styled(" ██████╗ ", accent),
            Span::styled("║", muted),
        ]),
        Line::from(vec![
            Span::styled("║  ", muted),
            Span::styled("╚══██╔══╝", accent),
            Span::styled("██╔════╝", accent),
            Span::styled("██╔════╝", accent),
            Span::styled("╚══██╔══╝", accent),
            Span::styled("██║   ██║", accent),
            Span::styled("██╔══██╗", accent),
            Span::styled("██╔═══██╗", accent),
            Span::styled("║", muted),
        ]),
        Line::from(vec![
            Span::styled("║     ", muted),
            Span::styled("██║   ", accent),
            Span::styled("█████╗  ", accent),
            Span::styled("███████╗", accent),
            Span::styled("   ██║   ", accent),
            Span::styled("██║   ██║", accent),
            Span::styled("██║  ██║", accent),
            Span::styled("██║   ██║", accent),
            Span::styled("║", muted),
        ]),
        Line::from(vec![
            Span::styled("║     ", muted),
            Span::styled("██║   ", accent),
            Span::styled("██╔══╝  ", accent),
            Span::styled("╚════██║", accent),
            Span::styled("   ██║   ", accent),
            Span::styled("██║   ██║", accent),
            Span::styled("██║  ██║", accent),
            Span::styled("██║   ██║", accent),
            Span::styled("║", muted),
        ]),
        Line::from(vec![
            Span::styled("║     ", muted),
            Span::styled("██║   ", accent),
            Span::styled("███████╗", accent),
            Span::styled("███████║", accent),
            Span::styled("   ██║   ", accent),
            Span::styled("╚██████╔╝", accent),
            Span::styled("██████╔╝", accent),
            Span::styled("╚██████╔╝", accent),
            Span::styled("║", muted),
        ]),
        Line::from(vec![
            Span::styled("║     ", muted),
            Span::styled("╚═╝   ", muted),
            Span::styled("╚══════╝", muted),
            Span::styled("╚══════╝", muted),
            Span::styled("   ╚═╝   ", muted),
            Span::styled(" ╚═════╝ ", muted),
            Span::styled("╚═════╝ ", muted),
            Span::styled(" ╚═════╝ ", muted),
            Span::styled("║", muted),
        ]),
        // spacer
        Line::from(Span::styled(
            "║                                                              ║",
            muted,
        )),
        // tagline
        Line::from(vec![
            Span::styled("║              ", muted),
            Span::styled("Autonomous Trading Agent", fg),
            Span::styled("  ·  ", muted),
            Span::styled(format!("v{}", VERSION), dim),
            Span::styled("             ║", muted),
        ]),
        // spacer
        Line::from(Span::styled(
            "║                                                              ║",
            muted,
        )),
        // ── Roman standard banner ──
        Line::from(vec![
            Span::styled("║              ", muted),
            Span::styled("⚔", gold),
            Span::styled("  ", muted),
            Span::styled("SENATVS · POPVLVSQVE · ROMANVS", gold),
            Span::styled("  ", muted),
            Span::styled("⚔", gold),
            Span::styled("              ║", muted),
        ]),
        // spacer
        Line::from(Span::styled(
            "║                                                              ║",
            muted,
        )),
        // ── Shield wall (testudo formation) ──
        Line::from(vec![
            Span::styled("║        ", muted),
            Span::styled("┌──────┬──────┬──────┬──────┬──────┬──────┐", gold),
            Span::styled("        ║", muted),
        ]),
        Line::from(vec![
            Span::styled("║   ⚔    ", muted),
            Span::styled("│", gold),
            Span::styled(" ▄▄▄▄ ", dim),
            Span::styled("│", gold),
            Span::styled(" ▄▄▄▄ ", dim),
            Span::styled("│", gold),
            Span::styled(" ▄▄▄▄ ", dim),
            Span::styled("│", gold),
            Span::styled(" ▄▄▄▄ ", dim),
            Span::styled("│", gold),
            Span::styled(" ▄▄▄▄ ", dim),
            Span::styled("│", gold),
            Span::styled(" ▄▄▄▄ ", dim),
            Span::styled("│", gold),
            Span::styled("    ⚔   ║", muted),
        ]),
        Line::from(vec![
            Span::styled("║  ╔╗     ", muted),
            Span::styled("│", gold),
            Span::styled(" ████ ", accent),
            Span::styled("│", gold),
            Span::styled(" ████ ", accent),
            Span::styled("│", gold),
            Span::styled(" ████ ", accent),
            Span::styled("│", gold),
            Span::styled(" ████ ", accent),
            Span::styled("│", gold),
            Span::styled(" ████ ", accent),
            Span::styled("│", gold),
            Span::styled(" ████ ", accent),
            Span::styled("│", gold),
            Span::styled("     ╔╗  ║", muted),
        ]),
        Line::from(vec![
            Span::styled("║  ╚╝     ", muted),
            Span::styled("│", gold),
            Span::styled(" ▀▀▀▀ ", dim),
            Span::styled("│", gold),
            Span::styled(" ▀▀▀▀ ", dim),
            Span::styled("│", gold),
            Span::styled(" ▀▀▀▀ ", dim),
            Span::styled("│", gold),
            Span::styled(" ▀▀▀▀ ", dim),
            Span::styled("│", gold),
            Span::styled(" ▀▀▀▀ ", dim),
            Span::styled("│", gold),
            Span::styled(" ▀▀▀▀ ", dim),
            Span::styled("│", gold),
            Span::styled("     ╚╝  ║", muted),
        ]),
        Line::from(vec![
            Span::styled("║        ", muted),
            Span::styled("└──────┴──────┴──────┴──────┴──────┴──────┘", gold),
            Span::styled("        ║", muted),
        ]),
        // spacer
        Line::from(Span::styled(
            "║                                                              ║",
            muted,
        )),
        // legionaries below shields
        Line::from(vec![
            Span::styled("║         ", muted),
            Span::styled("║║", danger),
            Span::styled("      ", muted),
            Span::styled("║║", danger),
            Span::styled("      ", muted),
            Span::styled("║║", danger),
            Span::styled("      ", muted),
            Span::styled("║║", danger),
            Span::styled("      ", muted),
            Span::styled("║║", danger),
            Span::styled("      ", muted),
            Span::styled("║║", danger),
            Span::styled("         ║", muted),
        ]),
        Line::from(vec![
            Span::styled("║         ", muted),
            Span::styled("╚╝", danger),
            Span::styled("      ", muted),
            Span::styled("╚╝", danger),
            Span::styled("      ", muted),
            Span::styled("╚╝", danger),
            Span::styled("      ", muted),
            Span::styled("╚╝", danger),
            Span::styled("      ", muted),
            Span::styled("╚╝", danger),
            Span::styled("      ", muted),
            Span::styled("╚╝", danger),
            Span::styled("         ║", muted),
        ]),
        // spacer
        Line::from(Span::styled(
            "║                                                              ║",
            muted,
        )),
        // prompt
        Line::from(vec![
            Span::styled("║                ", muted),
            Span::styled("Press any key", fg),
            Span::styled("                ║", muted),
        ]),
        // spacer
        Line::from(Span::styled(
            "║                                                              ║",
            muted,
        )),
        // ═══ bottom border ═══
        Line::from(Span::styled(
            "╚══════════════════════════════════════════════════════════════╝",
            muted,
        )),
    ];

    let text = Text::from(lines);
    let p = Paragraph::new(text);
    frame.render_widget(p, centered);
}
