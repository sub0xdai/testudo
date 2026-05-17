# Specification: JSON-LD Semantic API Layer

**Spec ID:** JNL-11-semantic-api
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / Interoperability
**Priority:** P3 — future-facing, enables AI/agent consumption
**Depends on:** JNL-05-journal-api, JNL-06-analytics-api
**Series:** Standalone — implements after core journal is complete

---

## Problem Statement

Testudo's API returns plain JSON. External AI agents, knowledge graphs, semantic tools, and interop systems (like meta-introspector's pastebin) cannot understand the meaning of the data without custom integration. By adding a JSON-LD context layer, Testudo's trade data becomes self-describing — any system that speaks semantic web can parse it without bespoke adapters.

This is the "headless dev API" angle: make trade data machine-interpretable, not just machine-readable.

---

## User Stories

- **As an AI agent**, I want to consume Testudo trade data with semantic context so I can reason about trading patterns without custom parsing.
- **As a developer**, I want to integrate Testudo journal data into a knowledge graph or analytics pipeline using standard semantic web tooling.
- **As a trader**, I want to export my trade data in a format that external AI tools can ingest for pattern analysis.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Define JSON-LD `@context` for all journal data types | High | semantic layer |
| FR-2 | Add `Accept: application/ld+json` content negotiation to journal endpoints | High | routes/journal.rs |
| FR-3 | When LD+JSON requested, wrap responses with `@context` and `@type` | High | middleware |
| FR-4 | Define custom vocabulary for trading concepts not in Schema.org | Medium | vocabulary |
| FR-5 | Provide a discoverable context document at `/api/v1/context.jsonld` | Medium | routes |
| FR-6 | Standard JSON responses unchanged (opt-in via Accept header) | High | middleware |

---

## Technical Implementation

### JSON-LD Context

Define a Testudo trading vocabulary that maps to Schema.org where possible and extends with custom terms:

```json
{
  "@context": {
    "@vocab": "https://testudo.trade/vocab#",
    "schema": "https://schema.org/",
    "xsd": "http://www.w3.org/2001/XMLSchema#",

    "Trade": "testudo:Trade",
    "JournalEntry": "testudo:JournalEntry",
    "TradingAccount": "schema:FinancialProduct",
    "PerformanceStats": "testudo:PerformanceStats",

    "symbol": "testudo:tradingSymbol",
    "side": "testudo:tradeSide",
    "entryPrice": { "@id": "testudo:entryPrice", "@type": "xsd:decimal" },
    "exitPrice": { "@id": "testudo:exitPrice", "@type": "xsd:decimal" },
    "quantity": { "@id": "schema:amount", "@type": "xsd:decimal" },
    "realizedPnl": { "@id": "testudo:realizedPnl", "@type": "xsd:decimal" },
    "netPnl": { "@id": "testudo:netPnl", "@type": "xsd:decimal" },
    "rMultiple": { "@id": "testudo:rMultiple", "@type": "xsd:decimal" },
    "fees": { "@id": "testudo:tradingFees", "@type": "xsd:decimal" },
    "exchange": "testudo:exchange",
    "leverage": { "@id": "testudo:leverage", "@type": "xsd:integer" },
    "openedAt": { "@id": "schema:startDate", "@type": "xsd:dateTime" },
    "closedAt": { "@id": "schema:endDate", "@type": "xsd:dateTime" },
    "durationSecs": { "@id": "schema:duration", "@type": "xsd:integer" },

    "winRate": { "@id": "testudo:winRate", "@type": "xsd:decimal" },
    "profitFactor": { "@id": "testudo:profitFactor", "@type": "xsd:decimal" },
    "maxDrawdown": { "@id": "testudo:maxDrawdown", "@type": "xsd:decimal" },
    "expectancy": { "@id": "testudo:expectancy", "@type": "xsd:decimal" },

    "body": "schema:text",
    "title": "schema:name",
    "tags": "schema:keywords",
    "dateCreated": "schema:dateCreated"
  }
}
```

### Content Negotiation Middleware

```rust
// crates/router/src/middleware/content_negotiation.rs

use actix_web::{HttpRequest, HttpResponse};

pub fn wants_jsonld(req: &HttpRequest) -> bool {
    req.headers()
        .get("Accept")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("application/ld+json"))
        .unwrap_or(false)
}

pub fn wrap_jsonld<T: Serialize>(data: T, type_name: &str) -> serde_json::Value {
    json!({
        "@context": "https://testudo.trade/api/v1/context.jsonld",
        "@type": type_name,
        "@id": format!("https://testudo.trade/api/v1/journal/{}", type_name.to_lowercase()),
        ...serde_json::to_value(data).unwrap()
    })
}
```

### Example Responses

**Standard JSON** (`Accept: application/json`):
```json
{
  "symbol": "BTC_USDT",
  "side": "LONG",
  "entry_price": "83412.00",
  "exit_price": "84200.00",
  "net_pnl": "45.30",
  "r_multiple": "2.1"
}
```

**JSON-LD** (`Accept: application/ld+json`):
```json
{
  "@context": "https://testudo.trade/api/v1/context.jsonld",
  "@type": "Trade",
  "@id": "urn:testudo:trade:550e8400-e29b-41d4-a716-446655440000",
  "symbol": "BTC_USDT",
  "side": "LONG",
  "entryPrice": "83412.00",
  "exitPrice": "84200.00",
  "netPnl": "45.30",
  "rMultiple": "2.1"
}
```

**Analytics overview as JSON-LD:**
```json
{
  "@context": "https://testudo.trade/api/v1/context.jsonld",
  "@type": "PerformanceStats",
  "winRate": "58.5",
  "profitFactor": "1.82",
  "expectancy": "15.12",
  "maxDrawdown": "456.00"
}
```

### Context Discovery Endpoint

```
GET /api/v1/context.jsonld
Content-Type: application/ld+json

Returns the full @context document
```

This allows any JSON-LD processor to resolve Testudo's vocabulary.

### Files

- `testudo-exchange/crates/router/src/middleware/content_negotiation.rs` — new
- `testudo-exchange/crates/router/src/routes/journal.rs` — add LD+JSON branch to handlers
- `testudo-exchange/crates/router/src/routes/context.rs` — serve context document
- `testudo-exchange/crates/router/src/main.rs` — register context route

---

## Acceptance Criteria

- [ ] `Accept: application/json` returns standard JSON (unchanged behavior)
- [ ] `Accept: application/ld+json` returns JSON-LD with `@context` and `@type`
- [ ] `/api/v1/context.jsonld` serves the vocabulary document
- [ ] JSON-LD responses validate with a JSON-LD processor (e.g., jsonld.js playground)
- [ ] All journal and analytics endpoints support content negotiation
- [ ] No performance impact on standard JSON responses
- [ ] Field names use camelCase in LD+JSON (standard JSON keeps snake_case)
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Future Extensions

- **RDFa HTML views**: Render trade data as HTML with embedded RDFa attributes for web crawlers
- **SPARQL endpoint**: Query trade data using semantic web query language
- **Linked Data Notifications**: Push trade events to subscribed AI agents
- **Schema.org integration**: Register Testudo vocabulary as a Schema.org extension

---

## Completion Signal

This spec is complete when:
1. AI agents can consume trade data via `Accept: application/ld+json`
2. Context document is discoverable and valid
3. Standard JSON consumers are unaffected
4. All acceptance criteria met
5. Code committed to master
