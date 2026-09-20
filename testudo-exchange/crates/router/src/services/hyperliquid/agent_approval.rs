//! AW-02: EIP-712 Approval Protocol
//!
//! Constructs EIP-712 typed data for the `approveAgent` action, submits
//! signed approvals to the Hyperliquid API, and verifies agent registration.
//!
//! The user signs the EIP-712 message with MetaMask (eth_signTypedData_v4),
//! then the backend forwards the signature to Hyperliquid. This ensures the
//! user's main private key never touches the server.

// @anchor exchange:router:agent_approval
// @tags api

use crate::types::hl_network::HlNetwork;
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApprovalError {
    #[error("Invalid signature format: {0}")]
    InvalidSignature(String),
    #[error("Hyperliquid API error: {0}")]
    ApiError(String),
    #[error("Agent not found in registration check")]
    RegistrationNotFound,
    #[error("HTTP request failed: {0}")]
    HttpError(String),
}

/// Build EIP-712 typed data JSON for MetaMask's `eth_signTypedData_v4`.
///
/// This constructs the exact JSON structure that MetaMask expects, matching
/// the Hyperliquid SDK's `ApproveAgent` action encoding.
pub fn build_eip712_typed_data(
    agent_address: &str,
    network: HlNetwork,
    nonce: u64,
) -> Value {
    let chain_str = network.chain_string();

    json!({
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "version", "type": "string" },
                { "name": "chainId", "type": "uint256" },
                { "name": "verifyingContract", "type": "address" }
            ],
            "HyperliquidTransaction:ApproveAgent": [
                { "name": "hyperliquidChain", "type": "string" },
                { "name": "agentAddress", "type": "address" },
                { "name": "agentName", "type": "string" },
                { "name": "nonce", "type": "uint64" }
            ]
        },
        "primaryType": "HyperliquidTransaction:ApproveAgent",
        "domain": {
            "name": "HyperliquidSignTransaction",
            "version": "1",
            "chainId": network.arbitrum_chain_id(),
            "verifyingContract": "0x0000000000000000000000000000000000000000"
        },
        "message": {
            "hyperliquidChain": chain_str,
            "agentAddress": agent_address,
            "agentName": null,
            "nonce": nonce
        }
    })
}

/// Parse a 65-byte ECDSA signature into (r, s, v) components.
///
/// Accepts both `0x`-prefixed and bare hex strings.
/// Returns (r_hex, s_hex, v) where r and s are 0x-prefixed 64-char hex.
pub fn parse_signature(sig_hex: &str) -> Result<(String, String, u8), ApprovalError> {
    let hex_str = sig_hex.strip_prefix("0x").unwrap_or(sig_hex);

    let sig_bytes = hex::decode(hex_str)
        .map_err(|e| ApprovalError::InvalidSignature(format!("invalid hex: {}", e)))?;

    if sig_bytes.len() != 65 {
        return Err(ApprovalError::InvalidSignature(format!(
            "expected 65 bytes, got {}",
            sig_bytes.len()
        )));
    }

    let r = format!("0x{}", hex::encode(&sig_bytes[0..32]));
    let s = format!("0x{}", hex::encode(&sig_bytes[32..64]));
    let v = sig_bytes[64];

    // MetaMask returns v as 27 or 28
    if v != 27 && v != 28 {
        return Err(ApprovalError::InvalidSignature(format!(
            "v must be 27 or 28, got {}",
            v
        )));
    }

    Ok((r, s, v))
}

/// Submit a signed approval to the Hyperliquid exchange API.
///
/// Assembles the full payload matching the SDK's `post()` format and
/// POSTs to the appropriate endpoint (mainnet or testnet).
pub async fn submit_approval(
    client: &reqwest::Client,
    agent_address: &str,
    network: HlNetwork,
    nonce: u64,
    signature: &str,
) -> Result<Value, ApprovalError> {
    let (r, s, v) = parse_signature(signature)?;
    let chain_str = network.chain_string();

    let payload = json!({
        "action": {
            "type": "approveAgent",
            "signatureChainId": format!("0x{:x}", network.arbitrum_chain_id()),
            "hyperliquidChain": chain_str,
            "agentAddress": agent_address,
            "agentName": null,
            "nonce": nonce
        },
        "nonce": nonce,
        "signature": {
            "r": r,
            "s": s,
            "v": v
        }
    });

    let url = network.exchange_url();
    let resp = client
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| ApprovalError::HttpError(e.to_string()))?;

    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| ApprovalError::HttpError(format!("failed to parse response: {}", e)))?;

    tracing::info!("HL approval response: HTTP {} body={}", status, body);

    if !status.is_success() {
        return Err(ApprovalError::ApiError(format!(
            "HTTP {}: {}",
            status,
            body
        )));
    }

    // Check for Hyperliquid-level errors in response.
    // HL returns {"status": "err", "response": "message"} on failure.
    if let Some(err) = body.get("error") {
        return Err(ApprovalError::ApiError(err.to_string()));
    }
    if body.get("status").and_then(|s| s.as_str()) == Some("err") {
        let msg = body
            .get("response")
            .and_then(|r| r.as_str())
            .unwrap_or("unknown error");
        return Err(ApprovalError::ApiError(msg.to_string()));
    }

    Ok(body)
}

/// Verify that an agent is registered by querying Hyperliquid's info API.
///
/// Calls the `extraAgents` endpoint and checks if the agent address
/// appears in the response.
pub async fn verify_registration(
    client: &reqwest::Client,
    wallet_address: &str,
    agent_address: &str,
    network: HlNetwork,
) -> Result<bool, ApprovalError> {
    let url = network.info_url();
    let payload = json!({
        "type": "extraAgents",
        "user": wallet_address
    });

    let resp = client
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| ApprovalError::HttpError(e.to_string()))?;

    let body: Value = resp
        .json()
        .await
        .map_err(|e| ApprovalError::HttpError(format!("failed to parse info response: {}", e)))?;

    tracing::info!("HL extraAgents response: {}", body);

    // Response is an array of agent objects; check if our agent appears
    let agent_lower = agent_address.to_lowercase();
    if let Some(agents) = body.as_array() {
        for agent in agents {
            if let Some(addr) = agent.get("address").and_then(|a| a.as_str()) {
                if addr.to_lowercase() == agent_lower {
                    return Ok(true);
                }
            }
            // Some responses use "agentAddress" key
            if let Some(addr) = agent.get("agentAddress").and_then(|a| a.as_str()) {
                if addr.to_lowercase() == agent_lower {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_eip712_typed_data_mainnet() {
        let agent_addr = "0x1234567890abcdef1234567890abcdef12345678";
        let nonce = 1710600000000u64;
        let data = build_eip712_typed_data(agent_addr, HlNetwork::Mainnet, nonce);

        // Verify domain
        let domain = &data["domain"];
        assert_eq!(domain["name"], "HyperliquidSignTransaction");
        assert_eq!(domain["version"], "1");
        // Mainnet uses Arbitrum One chainId for MetaMask-signed approvals
        assert_eq!(domain["chainId"], 42161);
        assert_eq!(
            domain["verifyingContract"],
            "0x0000000000000000000000000000000000000000"
        );

        // Verify message
        let msg = &data["message"];
        assert_eq!(msg["hyperliquidChain"], "Mainnet");
        assert_eq!(msg["agentAddress"], agent_addr);
        assert!(msg["agentName"].is_null());
        assert_eq!(msg["nonce"], nonce);

        // Verify primary type
        assert_eq!(
            data["primaryType"],
            "HyperliquidTransaction:ApproveAgent"
        );

        // Verify types include EIP712Domain and the action type
        assert!(data["types"]["EIP712Domain"].is_array());
        assert!(data["types"]["HyperliquidTransaction:ApproveAgent"].is_array());
    }

    #[test]
    fn test_build_eip712_typed_data_testnet() {
        let data = build_eip712_typed_data(
            "0xabcdef1234567890abcdef1234567890abcdef12",
            HlNetwork::Testnet,
            999999,
        );
        assert_eq!(data["message"]["hyperliquidChain"], "Testnet");
        // Testnet uses Arbitrum Sepolia chainId
        assert_eq!(data["domain"]["chainId"], 421614);
    }

    #[test]
    fn test_parse_signature_valid_with_prefix() {
        // 65 bytes: 32 (r) + 32 (s) + 1 (v=28=0x1c)
        let r_hex = "a".repeat(64);
        let s_hex = "b".repeat(64);
        let v_hex = "1c"; // 28
        let sig = format!("0x{}{}{}", r_hex, s_hex, v_hex);

        let (r, s, v) = parse_signature(&sig).unwrap();
        assert_eq!(r, format!("0x{}", r_hex));
        assert_eq!(s, format!("0x{}", s_hex));
        assert_eq!(v, 28);
    }

    #[test]
    fn test_parse_signature_valid_without_prefix() {
        let r_hex = "a".repeat(64);
        let s_hex = "b".repeat(64);
        let v_hex = "1b"; // 27
        let sig = format!("{}{}{}", r_hex, s_hex, v_hex);

        let (r, s, v) = parse_signature(&sig).unwrap();
        assert_eq!(r, format!("0x{}", r_hex));
        assert_eq!(s, format!("0x{}", s_hex));
        assert_eq!(v, 27);
    }

    #[test]
    fn test_parse_signature_wrong_length() {
        let err = parse_signature("0xabcdef").unwrap_err();
        assert!(matches!(err, ApprovalError::InvalidSignature(_)));
    }

    #[test]
    fn test_parse_signature_invalid_hex() {
        let err = parse_signature("0xzzzz").unwrap_err();
        assert!(matches!(err, ApprovalError::InvalidSignature(_)));
    }

    #[test]
    fn test_parse_signature_invalid_v() {
        // v = 0 (invalid, should be 27 or 28)
        let r_hex = "a".repeat(64);
        let s_hex = "b".repeat(64);
        let sig = format!("0x{}{}00", r_hex, s_hex);

        let err = parse_signature(&sig).unwrap_err();
        assert!(matches!(err, ApprovalError::InvalidSignature(_)));
    }
}
