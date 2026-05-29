// @anchor exchange:router:content_negotiation
// @tags api

use actix_web::HttpRequest;
use serde::Serialize;
use serde_json::Value;

const JSONLD_CONTEXT_URL: &str = "https://testudo.trade/api/v1/context.jsonld";

/// Check if the client requested JSON-LD via the Accept header.
pub fn wants_jsonld(req: &HttpRequest) -> bool {
    req.headers()
        .get("Accept")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("application/ld+json"))
        .unwrap_or(false)
}

/// Convert snake_case keys to camelCase for JSON-LD responses.
fn to_camel_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Recursively convert all keys in a JSON value from snake_case to camelCase.
fn convert_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (key, val) in map {
                new_map.insert(to_camel_case(&key), convert_keys(val));
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(convert_keys).collect()),
        other => other,
    }
}

/// Wrap a serializable value as a JSON-LD document with @context, @type, and optional @id.
pub fn wrap_jsonld<T: Serialize>(data: &T, type_name: &str, id: Option<String>) -> Value {
    let raw = serde_json::to_value(data).unwrap_or(Value::Null);
    let mut camel = convert_keys(raw);

    if let Value::Object(ref mut map) = camel {
        // Insert JSON-LD keywords at the top level
        map.insert("@context".to_string(), Value::String(JSONLD_CONTEXT_URL.to_string()));
        map.insert("@type".to_string(), Value::String(type_name.to_string()));
        if let Some(id_val) = id {
            map.insert("@id".to_string(), Value::String(id_val));
        }
    }

    camel
}

/// Wrap a list of items as a JSON-LD collection.
pub fn wrap_jsonld_collection<T: Serialize>(
    items: &[T],
    item_type: &str,
    collection_id: &str,
) -> Value {
    let members: Vec<Value> = items
        .iter()
        .map(|item| wrap_jsonld(item, item_type, None))
        .collect();

    serde_json::json!({
        "@context": JSONLD_CONTEXT_URL,
        "@type": "Collection",
        "@id": collection_id,
        "members": members,
        "totalItems": members.len(),
    })
}

/// Build an HttpResponse with the correct content type for JSON-LD.
pub fn jsonld_response(body: Value) -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("application/ld+json")
        .json(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("net_pnl"), "netPnl");
        assert_eq!(to_camel_case("entry_price"), "entryPrice");
        assert_eq!(to_camel_case("r_multiple"), "rMultiple");
        assert_eq!(to_camel_case("id"), "id");
        assert_eq!(to_camel_case("closed_at"), "closedAt");
        assert_eq!(to_camel_case("duration_secs"), "durationSecs");
        assert_eq!(to_camel_case("realized_pnl_pct"), "realizedPnlPct");
    }

    #[test]
    fn test_convert_keys_object() {
        let input = serde_json::json!({
            "net_pnl": "45.30",
            "entry_price": "83412.00",
            "nested_obj": {
                "some_field": 42
            }
        });
        let result = convert_keys(input);
        assert!(result.get("netPnl").is_some());
        assert!(result.get("entryPrice").is_some());
        assert!(result.get("nestedObj").unwrap().get("someField").is_some());
    }

    #[test]
    fn test_wrap_jsonld() {
        #[derive(Serialize)]
        struct TestTrade {
            net_pnl: String,
            symbol: String,
        }
        let trade = TestTrade {
            net_pnl: "45.30".to_string(),
            symbol: "BTC_USDT".to_string(),
        };
        let result = wrap_jsonld(&trade, "Trade", Some("urn:testudo:trade:abc".to_string()));
        assert_eq!(result["@type"], "Trade");
        assert_eq!(result["@context"], JSONLD_CONTEXT_URL);
        assert_eq!(result["@id"], "urn:testudo:trade:abc");
        assert_eq!(result["netPnl"], "45.30");
        assert_eq!(result["symbol"], "BTC_USDT");
    }

    #[test]
    fn test_wrap_jsonld_collection() {
        #[derive(Serialize)]
        struct Item {
            name: String,
        }
        let items = vec![
            Item { name: "a".to_string() },
            Item { name: "b".to_string() },
        ];
        let result = wrap_jsonld_collection(&items, "Tag", "urn:testudo:tags");
        assert_eq!(result["@type"], "Collection");
        assert_eq!(result["totalItems"], 2);
        assert!(result["members"].is_array());
    }
}
