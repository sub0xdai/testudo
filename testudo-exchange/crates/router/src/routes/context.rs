use actix_web::HttpResponse;
use serde_json::{json, Map, Value};

fn typed_term(id: &str, xsd_type: &str) -> Value {
    json!({ "@id": id, "@type": xsd_type })
}

/// Serve the JSON-LD context document describing the Testudo trading vocabulary.
/// GET /api/v1/context.jsonld
pub async fn get_context() -> HttpResponse {
    let mut ctx = Map::new();

    // Namespace prefixes
    ctx.insert("@vocab".into(), json!("https://testudo.trade/vocab#"));
    ctx.insert("schema".into(), json!("https://schema.org/"));
    ctx.insert("xsd".into(), json!("http://www.w3.org/2001/XMLSchema#"));
    ctx.insert("testudo".into(), json!("https://testudo.trade/vocab#"));

    // Types
    ctx.insert("Trade".into(), json!("testudo:Trade"));
    ctx.insert("JournalEntry".into(), json!("testudo:JournalEntry"));
    ctx.insert("Tag".into(), json!("testudo:Tag"));
    ctx.insert("Collection".into(), json!("testudo:Collection"));
    ctx.insert("TradingAccount".into(), json!("schema:FinancialProduct"));
    ctx.insert("PerformanceStats".into(), json!("testudo:PerformanceStats"));

    // Trade fields
    ctx.insert("symbol".into(), json!("testudo:tradingSymbol"));
    ctx.insert("side".into(), json!("testudo:tradeSide"));
    ctx.insert("exchange".into(), json!("testudo:exchange"));
    ctx.insert("entryPrice".into(), typed_term("testudo:entryPrice", "xsd:decimal"));
    ctx.insert("exitPrice".into(), typed_term("testudo:exitPrice", "xsd:decimal"));
    ctx.insert("quantity".into(), typed_term("schema:amount", "xsd:decimal"));
    ctx.insert("realizedPnl".into(), typed_term("testudo:realizedPnl", "xsd:decimal"));
    ctx.insert("realizedPnlPct".into(), typed_term("testudo:realizedPnlPct", "xsd:decimal"));
    ctx.insert("netPnl".into(), typed_term("testudo:netPnl", "xsd:decimal"));
    ctx.insert("rMultiple".into(), typed_term("testudo:rMultiple", "xsd:decimal"));
    ctx.insert("fees".into(), typed_term("testudo:tradingFees", "xsd:decimal"));
    ctx.insert("leverage".into(), typed_term("testudo:leverage", "xsd:integer"));
    ctx.insert("stopPrice".into(), typed_term("testudo:stopPrice", "xsd:decimal"));
    ctx.insert("targetPrice".into(), typed_term("testudo:targetPrice", "xsd:decimal"));
    ctx.insert("riskAmount".into(), typed_term("testudo:riskAmount", "xsd:decimal"));
    ctx.insert("openedAt".into(), typed_term("schema:startDate", "xsd:dateTime"));
    ctx.insert("closedAt".into(), typed_term("schema:endDate", "xsd:dateTime"));
    ctx.insert("durationSecs".into(), typed_term("schema:duration", "xsd:integer"));

    // Performance stats
    ctx.insert("winRate".into(), typed_term("testudo:winRate", "xsd:decimal"));
    ctx.insert("profitFactor".into(), typed_term("testudo:profitFactor", "xsd:decimal"));
    ctx.insert("maxDrawdown".into(), typed_term("testudo:maxDrawdown", "xsd:decimal"));
    ctx.insert("expectancy".into(), typed_term("testudo:expectancy", "xsd:decimal"));

    // Entry fields
    ctx.insert("title".into(), json!("schema:name"));
    ctx.insert("body".into(), json!("schema:text"));
    ctx.insert("tags".into(), json!("schema:keywords"));
    ctx.insert("entryType".into(), json!("testudo:entryType"));
    ctx.insert("entryDate".into(), typed_term("testudo:entryDate", "xsd:date"));
    ctx.insert("dateCreated".into(), json!("schema:dateCreated"));
    ctx.insert("dateModified".into(), json!("schema:dateModified"));
    ctx.insert("notes".into(), json!("schema:description"));
    ctx.insert("tradeId".into(), json!("testudo:tradeId"));
    ctx.insert("tradeGroupId".into(), json!("testudo:tradeGroupId"));

    // Collection fields
    ctx.insert("members".into(), json!("testudo:members"));
    ctx.insert("totalItems".into(), json!("testudo:totalItems"));

    let doc = json!({ "@context": Value::Object(ctx) });

    HttpResponse::Ok()
        .content_type("application/ld+json")
        .json(doc)
}
