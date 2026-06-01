// @anchor infra:cli:view:settings
// @tags ui

//! Settings screen — read-only config display with masked API keys.

use crate::config::Config;
use crate::theme::Theme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the settings screen.
pub fn render(frame: &mut Frame, theme: &Theme, area: Rect) {
    let config = Config::load();

    let lines = vec![
        Line::from(""),
        section_header("  Backend"),
        Line::from(""),
        kv("    Base URL", &config.api.base_url),
        kv("    Agent Key", &mask_key(&config.api.agent_key, "testudo_sk_")),
        kv("    WebSocket", &config.api.ws_url),
        Line::from(""),
        section_header("  LLM"),
        Line::from(""),
        kv("    Provider", &config.llm.provider),
        kv("    Model", &config.llm.model),
        kv("    API Key", &mask_key(&config.llm.api_key, "")),
        if let Some(ref url) = config.llm.base_url {
            kv("    Base URL", url)
        } else {
            Line::from("")
        },
        Line::from(""),
        section_header("  Agent"),
        Line::from(""),
        kv("    Loop Interval", &format!("{}s", config.agent.loop_interval_secs)),
        kv("    Shadow Mode", if config.agent.shadow_only { "true" } else { "false" }),
        Line::from(""),
        section_header("  UI"),
        Line::from(""),
        kv("    Theme", &config.ui.theme),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press Esc or q to return", Style::default().fg(theme.dim_fg)),
        ]),
    ];

    let text = Text::from(lines);
    let p = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Settings ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .title_style(Style::default().fg(theme.accent)),
        )
        .style(Style::default().fg(theme.fg));

    frame.render_widget(p, area);
}

fn section_header(label: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(label.to_string(), Style::default().fg(Color::Yellow)),
        Span::styled(" ──────────────────────────────────────────────", Style::default().fg(Color::DarkGray)),
    ])
}

fn kv(key: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(key.to_string(), Style::default().fg(Color::Gray)),
        Span::styled("  ", Style::default()),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ])
}

/// Mask an API key, showing first 4 and last 4 characters.
/// If the key has a known prefix (e.g., "sk-ant-"), preserves it and shows
/// 4 characters of the key content before masking.
pub fn mask_key(key: &str, prefix: &str) -> String {
    if key.is_empty() {
        return "(not set)".to_string();
    }
    let prefix_len = prefix.len();
    let visible_start = if prefix_len > 0 && key.starts_with(prefix) {
        prefix_len + 4 // show prefix + 4 chars of key
    } else {
        4 // show first 4 chars
    };
    if key.len() <= visible_start + 4 {
        // Key too short to mask meaningfully
        return format!("{}...", &key[..key.len().min(8)]);
    }
    let middle_len = key.len() - visible_start - 4;
    let dots = if middle_len > 4 { "····" } else { "****" };
    format!(
        "{}{}{}",
        &key[..visible_start],
        dots,
        &key[key.len() - 4..],
    )
}

use ratatui::style::Color;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_key_empty() {
        assert_eq!(mask_key("", ""), "(not set)");
    }

    #[test]
    fn mask_key_short() {
        assert_eq!(mask_key("abc", ""), "abc...");
    }

    #[test]
    fn mask_key_normal() {
        let masked = mask_key("sk-ant-api03-verylongkey1234abcd", "sk-ant-");
        assert!(masked.starts_with("sk-ant-api"));
        assert!(masked.ends_with("abcd"));
        assert!(masked.contains("····"));
    }

    #[test]
    fn mask_key_no_prefix() {
        let masked = mask_key("abcdefghijklmnop", "");
        assert!(masked.starts_with("abcd"));
        assert!(masked.ends_with("mnop"));
    }

    #[test]
    fn mask_key_testudo_sk_prefix() {
        let masked = mask_key("testudo_sk_abc123def456xyz789", "testudo_sk_");
        assert!(masked.starts_with("testudo_sk_abc1"));
        assert!(masked.ends_with("z789"));
    }
}
