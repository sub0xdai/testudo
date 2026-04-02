# Product Requirements Document (PRD)

**Project Name:** Testudo Extension
**Version:** 1.0 (Draft)
**Date:** February 8, 2026
**Status:** Approved for Development

## 1. Executive Summary
The Testudo extension is a browser-based interface designed to bridge TradingView charts with the Testudo execution engine. It allows traders to visually plan trades using native TradingView drawing tools and execute them via a hotkey.

**Key Differentiators:**
* **"Point-and-Shoot" Workflow:** Uses standard Long/Short drawing tools.
* **Backend Risk Logic:** Position sizing is calculated by the Rust engine, not the browser.
* **Safety First:** Includes a confirmation modal to prevent mis-clicks.

---

## 2. User Stories

### 2.1 Setup & Connection
* **US-1:** As a trader, I want to see a visual indicator (e.g., a green dot) in the extension icon showing that the WebSocket connection to the Testudo backend is active and healthy.
* **US-2:** As a trader, I want to configure the WebSocket URL (switching between `ws://localhost:8080` and a VPS address) via the extension settings page.

### 2.2 Trade Planning
* **US-3:** As a trader, I want to use the TradingView "Long Position" or "Short Position" tool to define my Entry, Stop Loss, and Take Profit levels on the chart.
* **US-4:** As a trader, I want to press a hotkey (Default: `Alt+X`) to initiate the trade sequence using the currently selected drawing tool.

### 2.3 Execution & Confirmation
* **US-5 (The Confirmation):** Upon pressing the hotkey, I want a modal to instantly appear displaying the extracted levels (Entry, Stop, Target) and the calculated R:R (Risk/Reward) ratio.
* **US-6:** As a trader, I want to press `ENTER` to confirm and send the order, or `ESC` to cancel.
* **US-7:** As a trader, I want the system to calculate the position size (Quantity) on the server side, based on my pre-defined risk parameters (e.g., 1% risk per trade), so I don't have to input manual quantities.

---

## 3. Functional Requirements

### 3.1 The Scraper (Content Script)
* **FR-01:** The system must identify the *currently active* drawing tool on the chart.
* **FR-02:** The system must scrape the following text values from the tool's floating DOM element:
    * **Entry Price**
    * **Stop Price**
    * **Target Price**
* **FR-03:** The system must identify the current **Ticker Symbol** (e.g., `BTCUSDT`) and **Timeframe** (e.g., `15m`) from the chart header.
* **FR-04:** The system must normalize scraped strings (e.g., removing commas, currency symbols) into valid floating-point numbers.

### 3.2 The Confirmation Modal (UI)
* **FR-05:** Triggered by `Alt+X`. Must overlay on top of the TradingView chart (Z-Index > 9999).
* **FR-06:** Must display: "LONG/SHORT on [SYMBOL]", "Entry", "Stop", "Target", and "R:R".
* **FR-07:** Must listen for `Key:Enter` (Execute) and `Key:Escape` (Dismiss).

### 3.3 The Communications (WebSocket)
* **FR-08:** The extension must maintain a persistent WebSocket connection to the configured backend URL.
* **FR-09:** On `Enter`, the extension must send a JSON payload (see Section 5).
* **FR-10:** The extension must display "Order Sent" or "Error: [Message]" based on the WebSocket ACK response from the backend.

### 3.4 The Backend Logic (Rust)
* **FR-11:** The backend must receive the trade parameters (Entry/Stop/Target).
* **FR-12:** The backend must calculate `Position Size` = `(Account_Balance * Risk_Percent) / (Entry - Stop_Loss)`.
* **FR-13:** The backend must route the calculated order to the connected exchange API.

---

## 4. Non-Functional Requirements

* **NFR-01 (Latency):** The time from `Enter` press to WebSocket message dispatch must be < 50ms.
* **NFR-02 (Security):** The WebSocket connection must support an API Key / Auth Token in the handshake headers (for future VPS deployment).
* **NFR-03 (Resilience):** The extension must automatically attempt to reconnect if the WebSocket connection drops (Exponential backoff).

---

## 5. Data Models

### 5.1 WebSocket Payload (Extension -> Backend)
This is the "Signal" message sent when you press Enter. Note that `quantity` is missing because the backend calculates it.

```json
{
  "type": "EXECUTE_TRADE",
  "payload": {
    "symbol": "BTCUSDT",
    "exchange": "BINANCE_FUTURES", 
    "side": "LONG",                
    "entry_price": 42500.50,
    "stop_loss": 41200.00,
    "take_profit": 45000.00,
    "timeframe": "15m",
    "timestamp": 1678886400000
  }
}
