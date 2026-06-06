//! AUTH-03: Centralized Policy Engine
//!
//! Single evaluation point for all agent key permission checks.
//! Route handlers call `PolicyEngine::authorize()` — no conditionals spread across routes.
//!
//! SIWE-authenticated users bypass the engine entirely (full access).
//! Only AgentKey auth_method hits policy evaluation.

// @anchor exchange:router:policy
// @tags api

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Permission ────────────────────────────────────────────────────────────

/// Permission scopes for agent API keys.
///
/// Each variant carries optional constraints — `None` means unrestricted.
/// Supports backward-compatible deserialization: old flat strings like
/// `"trade_execute"` deserialize as unparameterized (all `None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    /// Place trades (shadow or live)
    TradeExecute {
        /// None = all symbols allowed
        symbols: Option<Vec<String>>,
        /// None = all exchanges allowed
        exchanges: Option<Vec<String>>,
        /// Max risk per trade in USDT. None = no limit.
        max_risk_per_trade: Option<Decimal>,
        /// Max concurrent open positions. None = no limit.
        max_open_positions: Option<u32>,
    },
    /// Read journal data
    JournalRead {
        /// None = all tags visible
        tags: Option<Vec<String>>,
    },
    /// Write journal entries
    JournalWrite {
        /// None = can write any tag
        tags: Option<Vec<String>>,
    },
    /// Manage exchange accounts
    ExchangeManage {
        /// None = all exchanges
        exchanges: Option<Vec<String>>,
    },
    /// Modify risk configuration (binary — no parameters)
    RiskConfigure,
    /// Read account info (binary — no parameters)
    AccountRead,
}

/// Default permission set for trading agents.
/// Sufficient for the autonomous trading loop: signal + journal read/write.
pub fn default_permissions() -> Vec<Permission> {
    vec![
        Permission::TradeExecute {
            symbols: None,
            exchanges: None,
            max_risk_per_trade: None,
            max_open_positions: None,
        },
        Permission::JournalRead { tags: None },
        Permission::JournalWrite { tags: None },
        Permission::AccountRead,
    ]
}

// ── Serde: backward-compatible deserialization ────────────────────────────

impl Serialize for Permission {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(tag = "scope", rename_all = "snake_case")]
        enum PermissionObj<'a> {
            TradeExecute {
                #[serde(skip_serializing_if = "Option::is_none")]
                symbols: &'a Option<Vec<String>>,
                #[serde(skip_serializing_if = "Option::is_none")]
                exchanges: &'a Option<Vec<String>>,
                #[serde(skip_serializing_if = "Option::is_none")]
                max_risk_per_trade: &'a Option<Decimal>,
                #[serde(skip_serializing_if = "Option::is_none")]
                max_open_positions: &'a Option<u32>,
            },
            JournalRead {
                #[serde(skip_serializing_if = "Option::is_none")]
                tags: &'a Option<Vec<String>>,
            },
            JournalWrite {
                #[serde(skip_serializing_if = "Option::is_none")]
                tags: &'a Option<Vec<String>>,
            },
            ExchangeManage {
                #[serde(skip_serializing_if = "Option::is_none")]
                exchanges: &'a Option<Vec<String>>,
            },
            RiskConfigure,
            AccountRead,
        }

        let obj = match self {
            Permission::TradeExecute {
                symbols,
                exchanges,
                max_risk_per_trade,
                max_open_positions,
            } => PermissionObj::TradeExecute {
                symbols,
                exchanges,
                max_risk_per_trade,
                max_open_positions,
            },
            Permission::JournalRead { tags } => PermissionObj::JournalRead { tags },
            Permission::JournalWrite { tags } => PermissionObj::JournalWrite { tags },
            Permission::ExchangeManage { exchanges } => PermissionObj::ExchangeManage { exchanges },
            Permission::RiskConfigure => PermissionObj::RiskConfigure,
            Permission::AccountRead => PermissionObj::AccountRead,
        };
        obj.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Permission {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "scope", rename_all = "snake_case")]
        enum PermissionObj {
            TradeExecute {
                symbols: Option<Vec<String>>,
                exchanges: Option<Vec<String>>,
                max_risk_per_trade: Option<Decimal>,
                max_open_positions: Option<u32>,
            },
            JournalRead {
                tags: Option<Vec<String>>,
            },
            JournalWrite {
                tags: Option<Vec<String>>,
            },
            ExchangeManage {
                exchanges: Option<Vec<String>>,
            },
            RiskConfigure,
            AccountRead,
        }

        let value = serde_json::Value::deserialize(deserializer)?;

        // Backward compat: old flat string format (e.g., "trade_execute")
        if let Some(s) = value.as_str() {
            return match s {
                "trade_execute" => Ok(Permission::TradeExecute {
                    symbols: None,
                    exchanges: None,
                    max_risk_per_trade: None,
                    max_open_positions: None,
                }),
                "journal_read" => Ok(Permission::JournalRead { tags: None }),
                "journal_write" => Ok(Permission::JournalWrite { tags: None }),
                "exchange_manage" => Ok(Permission::ExchangeManage { exchanges: None }),
                "risk_configure" => Ok(Permission::RiskConfigure),
                "account_read" => Ok(Permission::AccountRead),
                other => Err(serde::de::Error::custom(format!(
                    "unknown permission scope: {}",
                    other
                ))),
            };
        }

        let obj: PermissionObj = serde_json::from_value(value).map_err(serde::de::Error::custom)?;

        Ok(match obj {
            PermissionObj::TradeExecute {
                symbols,
                exchanges,
                max_risk_per_trade,
                max_open_positions,
            } => Permission::TradeExecute {
                symbols,
                exchanges,
                max_risk_per_trade,
                max_open_positions,
            },
            PermissionObj::JournalRead { tags } => Permission::JournalRead { tags },
            PermissionObj::JournalWrite { tags } => Permission::JournalWrite { tags },
            PermissionObj::ExchangeManage { exchanges } => Permission::ExchangeManage { exchanges },
            PermissionObj::RiskConfigure => Permission::RiskConfigure,
            PermissionObj::AccountRead => Permission::AccountRead,
        })
    }
}

// ── Auth Method ───────────────────────────────────────────────────────────

/// How this request was authenticated.
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// Full-access SIWE/SIWS bearer token.
    Siwe,
    /// Scoped agent API key.
    AgentKey {
        key_id: Uuid,
        permissions: Vec<Permission>,
    },
}

/// Claims extracted from an agent key, stored in request extensions.
#[derive(Debug, Clone)]
pub struct AgentKeyClaims {
    pub user_id: Uuid,
    pub key_id: Uuid,
    pub permissions: Vec<Permission>,
}

// ── Action ────────────────────────────────────────────────────────────────

/// What the caller is trying to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Trade,
    JournalRead,
    JournalWrite,
    ExchangeManage,
    RiskConfigure,
    AccountRead,
}

// ── Action Context ────────────────────────────────────────────────────────

/// What the caller is operating on.
/// Fields left as `None` are skipped during evaluation.
#[derive(Debug, Default)]
pub struct ActionContext<'a> {
    pub symbol: Option<&'a str>,
    pub exchange: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub risk_amount: Option<Decimal>,
    pub open_position_count: Option<u32>,
}

// ── Policy Error ──────────────────────────────────────────────────────────

/// Typed authorization failure. Each variant carries enough data
/// for the route handler to return a specific 403 body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    MissingScope { required: &'static str },
    SymbolNotAllowed { symbol: String, allowed: Vec<String> },
    ExchangeNotAllowed { exchange: String, allowed: Vec<String> },
    RiskLimitExceeded { requested: Decimal, max: Decimal },
    MaxPositionsExceeded { current: u32, max: u32 },
    TagNotAllowed { tag: String, allowed: Vec<String> },
}

// ── Policy Engine ─────────────────────────────────────────────────────────

/// Central authorization evaluator.
///
/// SIWE-authenticated users should bypass the engine entirely
/// (the caller is responsible for the fast-path check).
/// Only `AgentKey` permissions are evaluated here.
pub struct PolicyEngine;

impl PolicyEngine {
    /// Evaluate whether `permissions` authorize `action` given `ctx`.
    ///
    /// Returns `Ok(())` if authorized, `Err(PolicyError)` with a specific
    /// reason if denied.
    pub fn authorize(
        permissions: &[Permission],
        action: Action,
        ctx: &ActionContext,
    ) -> Result<(), PolicyError> {
        match action {
            Action::Trade => Self::authorize_trade(permissions, ctx),
            Action::JournalRead => Self::authorize_journal_read(permissions, ctx),
            Action::JournalWrite => Self::authorize_journal_write(permissions, ctx),
            Action::ExchangeManage => Self::authorize_exchange_manage(permissions, ctx),
            Action::RiskConfigure => Self::authorize_binary(permissions, Permission::is_risk_configure),
            Action::AccountRead => Self::authorize_binary(permissions, Permission::is_account_read),
        }
    }

    fn authorize_trade(
        permissions: &[Permission],
        ctx: &ActionContext,
    ) -> Result<(), PolicyError> {
        let perm = permissions
            .iter()
            .find(|p| matches!(p, Permission::TradeExecute { .. }))
            .ok_or(PolicyError::MissingScope { required: "trade_execute" })?;

        if let Permission::TradeExecute {
            symbols,
            exchanges,
            max_risk_per_trade,
            max_open_positions,
        } = perm
        {
            if let (Some(allowed), Some(ctx_sym)) = (symbols, ctx.symbol) {
                if !allowed.iter().any(|s| s == ctx_sym) {
                    return Err(PolicyError::SymbolNotAllowed {
                        symbol: ctx_sym.to_string(),
                        allowed: allowed.clone(),
                    });
                }
            }
            if let (Some(allowed), Some(ctx_ex)) = (exchanges, ctx.exchange) {
                if !allowed.iter().any(|e| e == ctx_ex) {
                    return Err(PolicyError::ExchangeNotAllowed {
                        exchange: ctx_ex.to_string(),
                        allowed: allowed.clone(),
                    });
                }
            }
            if let (Some(max), Some(amount)) = (max_risk_per_trade, ctx.risk_amount) {
                if amount > *max {
                    return Err(PolicyError::RiskLimitExceeded {
                        requested: amount,
                        max: *max,
                    });
                }
            }
            if let (Some(max), Some(count)) = (max_open_positions, ctx.open_position_count) {
                if count >= *max {
                    return Err(PolicyError::MaxPositionsExceeded {
                        current: count,
                        max: *max,
                    });
                }
            }
        }
        Ok(())
    }

    fn authorize_journal_read(
        permissions: &[Permission],
        ctx: &ActionContext,
    ) -> Result<(), PolicyError> {
        let perm = permissions
            .iter()
            .find(|p| matches!(p, Permission::JournalRead { .. }))
            .ok_or(PolicyError::MissingScope { required: "journal_read" })?;

        if let Permission::JournalRead { tags } = perm {
            if let (Some(allowed), Some(ctx_tag)) = (tags, ctx.tag) {
                if !allowed.iter().any(|t| t == ctx_tag) {
                    return Err(PolicyError::TagNotAllowed {
                        tag: ctx_tag.to_string(),
                        allowed: allowed.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn authorize_journal_write(
        permissions: &[Permission],
        ctx: &ActionContext,
    ) -> Result<(), PolicyError> {
        let perm = permissions
            .iter()
            .find(|p| matches!(p, Permission::JournalWrite { .. }))
            .ok_or(PolicyError::MissingScope { required: "journal_write" })?;

        if let Permission::JournalWrite { tags } = perm {
            if let (Some(allowed), Some(ctx_tag)) = (tags, ctx.tag) {
                if !allowed.iter().any(|t| t == ctx_tag) {
                    return Err(PolicyError::TagNotAllowed {
                        tag: ctx_tag.to_string(),
                        allowed: allowed.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn authorize_exchange_manage(
        permissions: &[Permission],
        ctx: &ActionContext,
    ) -> Result<(), PolicyError> {
        let perm = permissions
            .iter()
            .find(|p| matches!(p, Permission::ExchangeManage { .. }))
            .ok_or(PolicyError::MissingScope { required: "exchange_manage" })?;

        if let Permission::ExchangeManage { exchanges } = perm {
            if let (Some(allowed), Some(ctx_ex)) = (exchanges, ctx.exchange) {
                if !allowed.iter().any(|e| e == ctx_ex) {
                    return Err(PolicyError::ExchangeNotAllowed {
                        exchange: ctx_ex.to_string(),
                        allowed: allowed.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Generic check for binary (parameterless) scopes.
    fn authorize_binary(
        permissions: &[Permission],
        predicate: fn(&Permission) -> bool,
    ) -> Result<(), PolicyError> {
        if permissions.iter().any(predicate) {
            Ok(())
        } else {
            Err(PolicyError::MissingScope { required: "scope" })
        }
    }
}

// ── Permission helpers ────────────────────────────────────────────────────

impl Permission {
    pub fn is_trade_execute(&self) -> bool {
        matches!(self, Permission::TradeExecute { .. })
    }

    pub fn is_journal_read(&self) -> bool {
        matches!(self, Permission::JournalRead { .. })
    }

    pub fn is_journal_write(&self) -> bool {
        matches!(self, Permission::JournalWrite { .. })
    }

    pub fn is_exchange_manage(&self) -> bool {
        matches!(self, Permission::ExchangeManage { .. })
    }

    pub fn is_risk_configure(&self) -> bool {
        matches!(self, Permission::RiskConfigure)
    }

    pub fn is_account_read(&self) -> bool {
        matches!(self, Permission::AccountRead)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // Helper to build a full-access TradeExecute permission
    fn trade_all() -> Permission {
        Permission::TradeExecute {
            symbols: None,
            exchanges: None,
            max_risk_per_trade: None,
            max_open_positions: None,
        }
    }

    // ── TradeExecute ──────────────────────────────────────────────────

    #[test]
    fn trade_passes_with_full_permission() {
        let perms = vec![trade_all()];
        let ctx = ActionContext {
            symbol: Some("BTC_USDT"),
            risk_amount: Some(dec!(500)),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ctx);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn trade_fails_without_permission() {
        let perms = vec![Permission::AccountRead];
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ActionContext::default());
        assert!(matches!(result, Err(PolicyError::MissingScope { .. })));
    }

    #[test]
    fn trade_fails_with_wrong_symbol() {
        let perms = vec![Permission::TradeExecute {
            symbols: Some(vec!["ETH_USDT".into()]),
            exchanges: None,
            max_risk_per_trade: None,
            max_open_positions: None,
        }];
        let ctx = ActionContext {
            symbol: Some("BTC_USDT"),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ctx);
        assert!(matches!(result, Err(PolicyError::SymbolNotAllowed { .. })));
    }

    #[test]
    fn trade_passes_with_allowed_symbol() {
        let perms = vec![Permission::TradeExecute {
            symbols: Some(vec!["BTC_USDT".into(), "ETH_USDT".into()]),
            exchanges: None,
            max_risk_per_trade: None,
            max_open_positions: None,
        }];
        let ctx = ActionContext {
            symbol: Some("BTC_USDT"),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn trade_passes_when_no_symbol_provided() {
        let perms = vec![Permission::TradeExecute {
            symbols: Some(vec!["BTC_USDT".into()]),
            exchanges: None,
            max_risk_per_trade: None,
            max_open_positions: None,
        }];
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ActionContext::default());
        assert!(result.is_ok(), "None ctx.symbol should skip symbol check");
    }

    #[test]
    fn trade_fails_with_wrong_exchange() {
        let perms = vec![Permission::TradeExecute {
            symbols: None,
            exchanges: Some(vec!["binance".into()]),
            max_risk_per_trade: None,
            max_open_positions: None,
        }];
        let ctx = ActionContext {
            exchange: Some("bybit"),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ctx);
        assert!(matches!(result, Err(PolicyError::ExchangeNotAllowed { .. })));
    }

    #[test]
    fn trade_passes_with_allowed_exchange() {
        let perms = vec![Permission::TradeExecute {
            symbols: None,
            exchanges: Some(vec!["binance".into(), "bybit".into()]),
            max_risk_per_trade: None,
            max_open_positions: None,
        }];
        let ctx = ActionContext {
            exchange: Some("binance"),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn trade_fails_when_risk_limit_exceeded() {
        let perms = vec![Permission::TradeExecute {
            symbols: None,
            exchanges: None,
            max_risk_per_trade: Some(dec!(500)),
            max_open_positions: None,
        }];
        let ctx = ActionContext {
            risk_amount: Some(dec!(501)),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ctx);
        assert!(matches!(result, Err(PolicyError::RiskLimitExceeded { .. })));
    }

    #[test]
    fn trade_passes_when_risk_under_limit() {
        let perms = vec![Permission::TradeExecute {
            symbols: None,
            exchanges: None,
            max_risk_per_trade: Some(dec!(500)),
            max_open_positions: None,
        }];
        let ctx = ActionContext {
            risk_amount: Some(dec!(500)),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ctx);
        assert!(result.is_ok(), "risk at limit should pass");
    }

    #[test]
    fn trade_fails_when_at_max_positions() {
        let perms = vec![Permission::TradeExecute {
            symbols: None,
            exchanges: None,
            max_risk_per_trade: None,
            max_open_positions: Some(3),
        }];
        let ctx = ActionContext {
            open_position_count: Some(3),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ctx);
        assert!(matches!(result, Err(PolicyError::MaxPositionsExceeded { .. })));
    }

    #[test]
    fn trade_passes_when_under_max_positions() {
        let perms = vec![Permission::TradeExecute {
            symbols: None,
            exchanges: None,
            max_risk_per_trade: None,
            max_open_positions: Some(3),
        }];
        let ctx = ActionContext {
            open_position_count: Some(2),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::Trade, &ctx);
        assert!(result.is_ok());
    }

    // ── JournalRead ───────────────────────────────────────────────────

    #[test]
    fn journal_read_passes_with_full_permission() {
        let perms = vec![Permission::JournalRead { tags: None }];
        let ctx = ActionContext {
            tag: Some("#momentum"),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::JournalRead, &ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn journal_read_fails_without_permission() {
        let perms = vec![Permission::AccountRead];
        let result = PolicyEngine::authorize(
            &perms,
            Action::JournalRead,
            &ActionContext::default(),
        );
        assert!(matches!(result, Err(PolicyError::MissingScope { .. })));
    }

    #[test]
    fn journal_read_fails_with_wrong_tag() {
        let perms = vec![Permission::JournalRead {
            tags: Some(vec!["#momentum".into()]),
        }];
        let ctx = ActionContext {
            tag: Some("#breakout"),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::JournalRead, &ctx);
        assert!(matches!(result, Err(PolicyError::TagNotAllowed { .. })));
    }

    #[test]
    fn journal_read_passes_with_allowed_tag() {
        let perms = vec![Permission::JournalRead {
            tags: Some(vec!["#momentum".into(), "#breakout".into()]),
        }];
        let ctx = ActionContext {
            tag: Some("#momentum"),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::JournalRead, &ctx);
        assert!(result.is_ok());
    }

    // ── JournalWrite ──────────────────────────────────────────────────

    #[test]
    fn journal_write_fails_without_permission() {
        let perms = vec![Permission::AccountRead];
        let result = PolicyEngine::authorize(
            &perms,
            Action::JournalWrite,
            &ActionContext::default(),
        );
        assert!(matches!(result, Err(PolicyError::MissingScope { .. })));
    }

    // ── ExchangeManage ────────────────────────────────────────────────

    #[test]
    fn exchange_manage_fails_without_permission() {
        let perms = vec![Permission::AccountRead];
        let result = PolicyEngine::authorize(
            &perms,
            Action::ExchangeManage,
            &ActionContext::default(),
        );
        assert!(matches!(result, Err(PolicyError::MissingScope { .. })));
    }

    #[test]
    fn exchange_manage_passes_with_allowed_exchange() {
        let perms = vec![Permission::ExchangeManage {
            exchanges: Some(vec!["binance".into()]),
        }];
        let ctx = ActionContext {
            exchange: Some("binance"),
            ..Default::default()
        };
        let result = PolicyEngine::authorize(&perms, Action::ExchangeManage, &ctx);
        assert!(result.is_ok());
    }

    // ── Binary scopes ─────────────────────────────────────────────────

    #[test]
    fn risk_configure_passes_when_present() {
        let perms = vec![Permission::RiskConfigure];
        let result = PolicyEngine::authorize(&perms, Action::RiskConfigure, &ActionContext::default());
        assert!(result.is_ok());
    }

    #[test]
    fn risk_configure_fails_when_absent() {
        let perms = vec![Permission::AccountRead];
        let result = PolicyEngine::authorize(&perms, Action::RiskConfigure, &ActionContext::default());
        assert!(matches!(result, Err(PolicyError::MissingScope { .. })));
    }

    #[test]
    fn account_read_passes_when_present() {
        let perms = vec![Permission::AccountRead];
        let result = PolicyEngine::authorize(&perms, Action::AccountRead, &ActionContext::default());
        assert!(result.is_ok());
    }

    #[test]
    fn account_read_fails_when_absent() {
        let perms = vec![Permission::RiskConfigure];
        let result = PolicyEngine::authorize(&perms, Action::AccountRead, &ActionContext::default());
        assert!(matches!(result, Err(PolicyError::MissingScope { .. })));
    }

    // ── Backward-compat deserialization ───────────────────────────────

    #[test]
    fn deserialize_old_flat_string() {
        let json = r#""trade_execute""#;
        let perm: Permission = serde_json::from_str(json).unwrap();
        assert_eq!(
            perm,
            Permission::TradeExecute {
                symbols: None,
                exchanges: None,
                max_risk_per_trade: None,
                max_open_positions: None,
            }
        );
    }

    #[test]
    fn deserialize_old_flat_array() {
        let json = r#"["trade_execute", "journal_read", "account_read"]"#;
        let perms: Vec<Permission> = serde_json::from_str(json).unwrap();
        assert_eq!(perms.len(), 3);
        assert!(matches!(perms[0], Permission::TradeExecute { .. }));
        assert!(matches!(perms[1], Permission::JournalRead { .. }));
        assert!(matches!(perms[2], Permission::AccountRead));
    }

    #[test]
    fn deserialize_new_parameterized() {
        let json = r#"{
            "scope": "trade_execute",
            "symbols": ["BTC_USDT", "ETH_USDT"],
            "exchanges": ["binance"],
            "max_risk_per_trade": "500",
            "max_open_positions": 3
        }"#;
        let perm: Permission = serde_json::from_str(json).unwrap();
        assert_eq!(
            perm,
            Permission::TradeExecute {
                symbols: Some(vec!["BTC_USDT".into(), "ETH_USDT".into()]),
                exchanges: Some(vec!["binance".into()]),
                max_risk_per_trade: Some(dec!(500)),
                max_open_positions: Some(3),
            }
        );
    }

    #[test]
    fn deserialize_mixed_old_and_new() {
        let json = r##"[
            "trade_execute",
            {"scope": "journal_read", "tags": ["#momentum"]},
            "account_read"
        ]"##;
        let perms: Vec<Permission> = serde_json::from_str(json).unwrap();
        assert_eq!(perms.len(), 3);
        // First: old flat string → unparameterized
        assert_eq!(
            perms[0],
            Permission::TradeExecute {
                symbols: None,
                exchanges: None,
                max_risk_per_trade: None,
                max_open_positions: None,
            }
        );
        // Second: new parameterized
        assert_eq!(
            perms[1],
            Permission::JournalRead {
                tags: Some(vec!["#momentum".into()]),
            }
        );
        // Third: old flat string → binary
        assert_eq!(perms[2], Permission::AccountRead);
    }

    #[test]
    fn deserialize_binary_scopes_from_string() {
        assert_eq!(
            serde_json::from_str::<Permission>(r#""risk_configure""#).unwrap(),
            Permission::RiskConfigure,
        );
        assert_eq!(
            serde_json::from_str::<Permission>(r#""account_read""#).unwrap(),
            Permission::AccountRead,
        );
    }

    #[test]
    fn deserialize_binary_scopes_from_object() {
        assert_eq!(
            serde_json::from_str::<Permission>(r#"{"scope": "risk_configure"}"#).unwrap(),
            Permission::RiskConfigure,
        );
        assert_eq!(
            serde_json::from_str::<Permission>(r#"{"scope": "account_read"}"#).unwrap(),
            Permission::AccountRead,
        );
    }

    // ── Serialization round-trip ──────────────────────────────────────

    #[test]
    fn serialize_then_deserialize_preserves_permission() {
        let original = Permission::TradeExecute {
            symbols: Some(vec!["BTC_USDT".into()]),
            exchanges: Some(vec!["binance".into()]),
            max_risk_per_trade: Some(dec!(500)),
            max_open_positions: Some(3),
        };
        let json = serde_json::to_string(&original).unwrap();
        let round_tripped: Permission = serde_json::from_str(&json).unwrap();
        assert_eq!(original, round_tripped);
    }

    // ── default_permissions ───────────────────────────────────────────

    #[test]
    fn default_permissions_has_expected_scopes() {
        let perms = default_permissions();
        assert!(perms.iter().any(Permission::is_trade_execute));
        assert!(perms.iter().any(Permission::is_journal_read));
        assert!(perms.iter().any(Permission::is_journal_write));
        assert!(perms.iter().any(Permission::is_account_read));
        assert!(!perms.iter().any(Permission::is_risk_configure));
        assert!(!perms.iter().any(Permission::is_exchange_manage));
    }

    // ── PolicyError display ───────────────────────────────────────────

    #[test]
    fn policy_error_contains_relevant_info() {
        let err = PolicyError::SymbolNotAllowed {
            symbol: "BTC_USDT".into(),
            allowed: vec!["ETH_USDT".into()],
        };
        let msg = format!("{:?}", err);
        assert!(msg.contains("BTC_USDT"));
        assert!(msg.contains("ETH_USDT"));
    }
}
