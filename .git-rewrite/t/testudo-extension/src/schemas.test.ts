import { describe, it, expect } from "vitest";
import {
  SettingsSchema,
  StoredSettingsSchema,
  AuthTokensSchema,
  StoredTokensSchema,
  RefreshResponseSchema,
  ActiveExchangeStorageSchema,
  PairResponseSchema,
  JwtWalletPayloadSchema,
  JwtSubPayloadSchema,
  TradePayloadSchema,
  BackendResponseSchema,
  ErrorResponseSchema,
  TradeGroupResponseSchema,
  TradeListResponseSchema,
  ExchangeInfoSchema,
  ListExchangesResponseSchema,
  ExchangeAccountSchema,
  ExchangeAccountsResponseSchema,
  AddExchangeAccountResponseSchema,
  TestConnectionResultSchema,
  ExchangeBalanceApiResponseSchema,
  ExchangePositionSchema,
  ExchangeOpenOrderSchema,
  ExchangePositionsApiResponseSchema,
  SidecarHealthResponseSchema,
  WebSocketMessageSchema,
  SidecarStreamDataSchema,
  RuntimeMessageSchema,
} from "./schemas";

// --- SettingsSchema ---

describe("SettingsSchema", () => {
  it("accepts valid settings with URL strings", () => {
    const result = SettingsSchema.safeParse({
      backendUrl: "http://localhost:8080",
      wsUrl: "ws://127.0.0.1:4000",
    });
    expect(result.success).toBe(true);
  });

  it("accepts https/wss URLs", () => {
    const result = SettingsSchema.safeParse({
      backendUrl: "https://api.testudo.io",
      wsUrl: "wss://ws.testudo.io",
    });
    expect(result.success).toBe(true);
  });

  it("rejects missing backendUrl", () => {
    const result = SettingsSchema.safeParse({ wsUrl: "ws://127.0.0.1:4000" });
    expect(result.success).toBe(false);
  });

  it("rejects missing wsUrl", () => {
    const result = SettingsSchema.safeParse({ backendUrl: "http://localhost:8080" });
    expect(result.success).toBe(false);
  });

  it("rejects empty object", () => {
    const result = SettingsSchema.safeParse({});
    expect(result.success).toBe(false);
  });

  it("rejects non-URL strings", () => {
    const result = SettingsSchema.safeParse({
      backendUrl: "not-a-url",
      wsUrl: "also-not-a-url",
    });
    expect(result.success).toBe(false);
  });

  it("rejects numeric values", () => {
    const result = SettingsSchema.safeParse({
      backendUrl: 8080,
      wsUrl: 4000,
    });
    expect(result.success).toBe(false);
  });
});

// --- StoredSettingsSchema ---

describe("StoredSettingsSchema", () => {
  it("accepts full settings", () => {
    const result = StoredSettingsSchema.safeParse({
      backendUrl: "http://localhost:8080",
      wsUrl: "ws://127.0.0.1:4000",
    });
    expect(result.success).toBe(true);
  });

  it("accepts empty object (both optional)", () => {
    const result = StoredSettingsSchema.safeParse({});
    expect(result.success).toBe(true);
  });

  it("accepts partial with only backendUrl", () => {
    const result = StoredSettingsSchema.safeParse({
      backendUrl: "http://localhost:8080",
    });
    expect(result.success).toBe(true);
  });

  it("accepts partial with only wsUrl", () => {
    const result = StoredSettingsSchema.safeParse({
      wsUrl: "ws://127.0.0.1:4000",
    });
    expect(result.success).toBe(true);
  });

  it("rejects non-URL string when provided", () => {
    const result = StoredSettingsSchema.safeParse({
      backendUrl: "not-a-url",
    });
    expect(result.success).toBe(false);
  });
});

// --- AuthTokensSchema ---

describe("AuthTokensSchema", () => {
  it("accepts valid auth tokens", () => {
    const result = AuthTokensSchema.safeParse({
      access_token: "eyJhbGciOiJIUzI1NiJ9.test",
      refresh_token: "refresh-abc-123",
      expires_in: 3600,
    });
    expect(result.success).toBe(true);
  });

  it("rejects empty access_token", () => {
    const result = AuthTokensSchema.safeParse({
      access_token: "",
      refresh_token: "refresh-abc-123",
      expires_in: 3600,
    });
    expect(result.success).toBe(false);
  });

  it("rejects empty refresh_token", () => {
    const result = AuthTokensSchema.safeParse({
      access_token: "valid-token",
      refresh_token: "",
      expires_in: 3600,
    });
    expect(result.success).toBe(false);
  });

  it("rejects non-integer expires_in", () => {
    const result = AuthTokensSchema.safeParse({
      access_token: "valid-token",
      refresh_token: "refresh-token",
      expires_in: 3600.5,
    });
    expect(result.success).toBe(false);
  });

  it("rejects string expires_in", () => {
    const result = AuthTokensSchema.safeParse({
      access_token: "valid-token",
      refresh_token: "refresh-token",
      expires_in: "3600",
    });
    expect(result.success).toBe(false);
  });

  it("rejects missing fields", () => {
    const result = AuthTokensSchema.safeParse({ access_token: "test" });
    expect(result.success).toBe(false);
  });
});

// --- StoredTokensSchema ---

describe("StoredTokensSchema", () => {
  it("accepts valid stored tokens", () => {
    const result = StoredTokensSchema.safeParse({
      accessToken: "abc",
      refreshToken: "def",
      tokenExpiry: 1700000000,
    });
    expect(result.success).toBe(true);
  });

  it("accepts without optional tokenExpiry", () => {
    const result = StoredTokensSchema.safeParse({
      accessToken: "abc",
      refreshToken: "def",
    });
    expect(result.success).toBe(true);
  });

  it("accepts empty strings (no min constraint)", () => {
    const result = StoredTokensSchema.safeParse({
      accessToken: "",
      refreshToken: "",
    });
    expect(result.success).toBe(true);
  });

  it("rejects missing accessToken", () => {
    const result = StoredTokensSchema.safeParse({ refreshToken: "def" });
    expect(result.success).toBe(false);
  });
});

// --- RefreshResponseSchema ---

describe("RefreshResponseSchema", () => {
  it("accepts valid refresh response with nested tokens", () => {
    const result = RefreshResponseSchema.safeParse({
      tokens: {
        access_token: "new-token",
        refresh_token: "new-refresh",
        expires_in: 7200,
      },
    });
    expect(result.success).toBe(true);
  });

  it("rejects when tokens field is missing", () => {
    const result = RefreshResponseSchema.safeParse({});
    expect(result.success).toBe(false);
  });

  it("rejects when nested tokens are invalid", () => {
    const result = RefreshResponseSchema.safeParse({
      tokens: { access_token: "" },
    });
    expect(result.success).toBe(false);
  });
});

// --- ActiveExchangeStorageSchema ---

describe("ActiveExchangeStorageSchema", () => {
  it("accepts with activeExchangeId", () => {
    const result = ActiveExchangeStorageSchema.safeParse({
      activeExchangeId: "exchange-123",
    });
    expect(result.success).toBe(true);
  });

  it("accepts without activeExchangeId (optional)", () => {
    const result = ActiveExchangeStorageSchema.safeParse({});
    expect(result.success).toBe(true);
  });

  it("accepts undefined activeExchangeId", () => {
    const result = ActiveExchangeStorageSchema.safeParse({
      activeExchangeId: undefined,
    });
    expect(result.success).toBe(true);
  });
});

// --- PairResponseSchema ---

describe("PairResponseSchema", () => {
  it("accepts valid pair response", () => {
    const result = PairResponseSchema.safeParse({
      user: {
        id: "user-abc-123",
        wallet_address: "0x1234567890abcdef",
      },
      tokens: {
        access_token: "token-abc",
        refresh_token: "refresh-abc",
        expires_in: 3600,
      },
    });
    expect(result.success).toBe(true);
  });

  it("rejects empty user id", () => {
    const result = PairResponseSchema.safeParse({
      user: {
        id: "",
        wallet_address: "0xabc",
      },
      tokens: {
        access_token: "t",
        refresh_token: "r",
        expires_in: 3600,
      },
    });
    expect(result.success).toBe(false);
  });

  it("rejects empty wallet_address", () => {
    const result = PairResponseSchema.safeParse({
      user: {
        id: "user-1",
        wallet_address: "",
      },
      tokens: {
        access_token: "t",
        refresh_token: "r",
        expires_in: 3600,
      },
    });
    expect(result.success).toBe(false);
  });

  it("rejects missing user field", () => {
    const result = PairResponseSchema.safeParse({
      tokens: {
        access_token: "t",
        refresh_token: "r",
        expires_in: 3600,
      },
    });
    expect(result.success).toBe(false);
  });
});

// --- JwtWalletPayloadSchema ---

describe("JwtWalletPayloadSchema", () => {
  it("accepts valid wallet address", () => {
    const result = JwtWalletPayloadSchema.safeParse({
      wallet_address: "0xabc123",
    });
    expect(result.success).toBe(true);
  });

  it("accepts missing wallet_address (optional)", () => {
    const result = JwtWalletPayloadSchema.safeParse({});
    expect(result.success).toBe(true);
  });

  it("rejects empty wallet_address (min 1)", () => {
    const result = JwtWalletPayloadSchema.safeParse({
      wallet_address: "",
    });
    expect(result.success).toBe(false);
  });
});

// --- JwtSubPayloadSchema ---

describe("JwtSubPayloadSchema", () => {
  it("accepts valid sub", () => {
    const result = JwtSubPayloadSchema.safeParse({ sub: "user-123" });
    expect(result.success).toBe(true);
  });

  it("accepts missing sub (optional)", () => {
    const result = JwtSubPayloadSchema.safeParse({});
    expect(result.success).toBe(true);
  });

  it("rejects empty sub (min 1)", () => {
    const result = JwtSubPayloadSchema.safeParse({ sub: "" });
    expect(result.success).toBe(false);
  });
});

// --- TradePayloadSchema ---

describe("TradePayloadSchema", () => {
  const validPayload = {
    symbol: "BTC_USDT",
    side: "LONG" as const,
    entry: 65000,
    stop: 63000,
    target: 70000,
    timeframe: "15m",
    management: {
      risk_percent: 1.0,
      break_even_at: 50,
      trailing_stop: { enabled: false, distance_percent: 25 },
      partial_tp: { enabled: false, close_percent: 50 },
    },
  };

  it("accepts valid LONG trade payload", () => {
    const result = TradePayloadSchema.safeParse(validPayload);
    expect(result.success).toBe(true);
  });

  it("accepts valid SHORT trade payload", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      side: "SHORT",
    });
    expect(result.success).toBe(true);
  });

  it("accepts optional exchange_account_id", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      exchange_account_id: "account-123",
    });
    expect(result.success).toBe(true);
  });

  it("accepts optional leverage", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, leverage: 10 },
    });
    expect(result.success).toBe(true);
  });

  it("defaults break_even_enabled to true when omitted", () => {
    const result = TradePayloadSchema.safeParse(validPayload);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.management.break_even_enabled).toBe(true);
    }
  });

  // --- Missing required fields ---

  it("rejects missing symbol", () => {
    const { symbol: _, ...rest } = validPayload;
    const result = TradePayloadSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it("rejects empty symbol", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, symbol: "" });
    expect(result.success).toBe(false);
  });

  it("rejects missing side", () => {
    const { side: _, ...rest } = validPayload;
    const result = TradePayloadSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it("rejects missing entry", () => {
    const { entry: _, ...rest } = validPayload;
    const result = TradePayloadSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it("rejects missing stop", () => {
    const { stop: _, ...rest } = validPayload;
    const result = TradePayloadSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it("rejects missing target", () => {
    const { target: _, ...rest } = validPayload;
    const result = TradePayloadSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it("rejects missing timeframe", () => {
    const { timeframe: _, ...rest } = validPayload;
    const result = TradePayloadSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  it("rejects missing management", () => {
    const { management: _, ...rest } = validPayload;
    const result = TradePayloadSchema.safeParse(rest);
    expect(result.success).toBe(false);
  });

  // --- Invalid side ---

  it("rejects invalid side value", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, side: "BUY" });
    expect(result.success).toBe(false);
  });

  it("rejects lowercase side", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, side: "long" });
    expect(result.success).toBe(false);
  });

  // --- Negative / zero prices ---

  it("rejects negative entry price", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, entry: -1 });
    expect(result.success).toBe(false);
  });

  it("rejects zero entry price", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, entry: 0 });
    expect(result.success).toBe(false);
  });

  it("rejects negative stop price", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, stop: -500 });
    expect(result.success).toBe(false);
  });

  it("rejects zero stop price", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, stop: 0 });
    expect(result.success).toBe(false);
  });

  it("rejects negative target price", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, target: -100 });
    expect(result.success).toBe(false);
  });

  it("rejects zero target price", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, target: 0 });
    expect(result.success).toBe(false);
  });

  // --- risk_percent boundaries ---

  it("accepts risk_percent at lower bound (0.1)", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, risk_percent: 0.1 },
    });
    expect(result.success).toBe(true);
  });

  it("rejects risk_percent below lower bound (0.09)", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, risk_percent: 0.09 },
    });
    expect(result.success).toBe(false);
  });

  it("accepts risk_percent at upper bound (100)", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, risk_percent: 100 },
    });
    expect(result.success).toBe(true);
  });

  it("rejects risk_percent above upper bound (100.1)", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, risk_percent: 100.1 },
    });
    expect(result.success).toBe(false);
  });

  it("accepts risk_percent at typical value (1.0)", () => {
    const result = TradePayloadSchema.safeParse(validPayload);
    expect(result.success).toBe(true);
  });

  // --- leverage boundaries ---

  it("accepts leverage at 1", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, leverage: 1 },
    });
    expect(result.success).toBe(true);
  });

  it("accepts leverage at 100", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, leverage: 100 },
    });
    expect(result.success).toBe(true);
  });

  it("rejects leverage at 0", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, leverage: 0 },
    });
    expect(result.success).toBe(false);
  });

  it("rejects leverage above 100", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, leverage: 101 },
    });
    expect(result.success).toBe(false);
  });

  // --- break_even_at boundaries ---

  it("accepts break_even_at at 0", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, break_even_at: 0 },
    });
    expect(result.success).toBe(true);
  });

  it("accepts break_even_at at 100", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, break_even_at: 100 },
    });
    expect(result.success).toBe(true);
  });

  it("rejects break_even_at below 0", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, break_even_at: -1 },
    });
    expect(result.success).toBe(false);
  });

  it("rejects break_even_at above 100", () => {
    const result = TradePayloadSchema.safeParse({
      ...validPayload,
      management: { ...validPayload.management, break_even_at: 101 },
    });
    expect(result.success).toBe(false);
  });

  // --- String type for numeric fields ---

  it("rejects string entry price", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, entry: "65000" });
    expect(result.success).toBe(false);
  });

  it("rejects string stop price", () => {
    const result = TradePayloadSchema.safeParse({ ...validPayload, stop: "63000" });
    expect(result.success).toBe(false);
  });
});

// --- BackendResponseSchema ---

describe("BackendResponseSchema", () => {
  it("accepts success response with data", () => {
    const result = BackendResponseSchema.safeParse({
      success: true,
      data: { order_id: "123" },
    });
    expect(result.success).toBe(true);
  });

  it("accepts error response with error string", () => {
    const result = BackendResponseSchema.safeParse({
      success: false,
      error: "Trade rejected",
    });
    expect(result.success).toBe(true);
  });

  it("accepts response with null error", () => {
    const result = BackendResponseSchema.safeParse({
      success: true,
      error: null,
    });
    expect(result.success).toBe(true);
  });

  it("accepts response with warnings array", () => {
    const result = BackendResponseSchema.safeParse({
      success: true,
      warnings: ["Low balance", "High leverage"],
    });
    expect(result.success).toBe(true);
  });

  it("accepts minimal success response", () => {
    const result = BackendResponseSchema.safeParse({ success: true });
    expect(result.success).toBe(true);
  });

  it("rejects missing success field", () => {
    const result = BackendResponseSchema.safeParse({ data: {} });
    expect(result.success).toBe(false);
  });
});

// --- ErrorResponseSchema ---

describe("ErrorResponseSchema", () => {
  it("accepts error with error field", () => {
    const result = ErrorResponseSchema.safeParse({ error: "Not found" });
    expect(result.success).toBe(true);
  });

  it("accepts error with message field", () => {
    const result = ErrorResponseSchema.safeParse({ message: "Internal error" });
    expect(result.success).toBe(true);
  });

  it("accepts empty object (both fields optional)", () => {
    const result = ErrorResponseSchema.safeParse({});
    expect(result.success).toBe(true);
  });

  it("accepts both fields present", () => {
    const result = ErrorResponseSchema.safeParse({
      error: "ERR_AUTH",
      message: "Token expired",
    });
    expect(result.success).toBe(true);
  });
});

// --- TradeGroupResponseSchema ---

describe("TradeGroupResponseSchema", () => {
  it("accepts valid trade group response", () => {
    const result = TradeGroupResponseSchema.safeParse({
      id: "group-123",
      symbol: "BTC_USDT",
      entry_order_id: "order-abc",
      entry_price: "65000.50",
      entry_quantity: "0.1",
      stop_loss_price: "63000",
      stop_loss_order_id: "order-sl",
      status: "active",
    });
    expect(result.success).toBe(true);
  });

  it("accepts numeric values for DecimalLikeString fields", () => {
    const result = TradeGroupResponseSchema.safeParse({
      id: "group-123",
      symbol: "BTC_USDT",
      entry_order_id: "order-abc",
      entry_price: 65000.5,
      entry_quantity: 0.1,
      status: "active",
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.entry_price).toBe("65000.5");
      expect(result.data.entry_quantity).toBe("0.1");
    }
  });

  it("defaults nullable fields to null", () => {
    const result = TradeGroupResponseSchema.safeParse({
      id: "group-123",
      symbol: "BTC_USDT",
      entry_order_id: "order-abc",
      entry_quantity: "0.1",
      status: "active",
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.entry_price).toBeNull();
      expect(result.data.stop_loss_price).toBeNull();
      expect(result.data.stop_loss_order_id).toBeNull();
      expect(result.data.take_profit_targets).toEqual([]);
    }
  });

  it("accepts take_profit_targets array", () => {
    const result = TradeGroupResponseSchema.safeParse({
      id: "group-123",
      symbol: "BTC_USDT",
      entry_order_id: "order-abc",
      entry_quantity: "0.1",
      status: "active",
      take_profit_targets: [
        { price: "70000", percent_to_close: "50", filled: false },
        { price: "75000", percent_to_close: "50", order_id: "tp-1", filled: true },
      ],
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.take_profit_targets).toHaveLength(2);
    }
  });

  it("defaults break_even fields to false", () => {
    const result = TradeGroupResponseSchema.safeParse({
      id: "group-123",
      symbol: "BTC_USDT",
      entry_order_id: "order-abc",
      entry_quantity: "0.1",
      status: "active",
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.break_even_enabled).toBe(false);
      expect(result.data.break_even_triggered).toBe(false);
    }
  });

  it("rejects missing id", () => {
    const result = TradeGroupResponseSchema.safeParse({
      symbol: "BTC_USDT",
      entry_order_id: "order-abc",
      entry_quantity: "0.1",
      status: "active",
    });
    expect(result.success).toBe(false);
  });
});

// --- TradeListResponseSchema ---

describe("TradeListResponseSchema", () => {
  it("accepts valid trade list", () => {
    const result = TradeListResponseSchema.safeParse({
      success: true,
      data: [
        {
          id: "g1",
          symbol: "BTC_USDT",
          entry_order_id: "o1",
          entry_quantity: "0.1",
          status: "active",
        },
      ],
    });
    expect(result.success).toBe(true);
  });

  it("accepts null data", () => {
    const result = TradeListResponseSchema.safeParse({
      success: true,
      data: null,
    });
    expect(result.success).toBe(true);
  });

  it("accepts missing data (optional)", () => {
    const result = TradeListResponseSchema.safeParse({ success: true });
    expect(result.success).toBe(true);
  });

  it("accepts error response", () => {
    const result = TradeListResponseSchema.safeParse({
      success: false,
      error: "Unauthorized",
    });
    expect(result.success).toBe(true);
  });
});

// --- ExchangeInfoSchema ---

describe("ExchangeInfoSchema", () => {
  it("accepts valid exchange info", () => {
    const result = ExchangeInfoSchema.safeParse({
      id: "binance",
      name: "Binance",
      type: "cex",
      description: "Binance exchange",
      supported_features: ["spot", "futures"],
      required_credentials: ["api_key", "secret"],
      optional_credentials: ["passphrase"],
    });
    expect(result.success).toBe(true);
  });

  it("accepts empty arrays for credentials", () => {
    const result = ExchangeInfoSchema.safeParse({
      id: "test",
      name: "Test",
      type: "cex",
      description: "Test exchange",
      supported_features: [],
      required_credentials: [],
      optional_credentials: [],
    });
    expect(result.success).toBe(true);
  });

  it("rejects missing required fields", () => {
    const result = ExchangeInfoSchema.safeParse({ id: "binance" });
    expect(result.success).toBe(false);
  });
});

// --- ListExchangesResponseSchema ---

describe("ListExchangesResponseSchema", () => {
  it("accepts valid exchange list", () => {
    const result = ListExchangesResponseSchema.safeParse({
      exchanges: [
        {
          id: "woo",
          name: "WOO",
          type: "cex",
          description: "WOO exchange",
          supported_features: ["futures"],
          required_credentials: ["api_key", "secret"],
          optional_credentials: [],
        },
      ],
    });
    expect(result.success).toBe(true);
  });

  it("accepts empty response", () => {
    const result = ListExchangesResponseSchema.safeParse({});
    expect(result.success).toBe(true);
  });

  it("accepts error response", () => {
    const result = ListExchangesResponseSchema.safeParse({
      error: "Failed to fetch",
    });
    expect(result.success).toBe(true);
  });
});

// --- ExchangeAccountSchema ---

describe("ExchangeAccountSchema", () => {
  it("accepts valid exchange account", () => {
    const result = ExchangeAccountSchema.safeParse({
      id: "acc-123",
      exchange_name: "binance",
      account_name: "Main Account",
      is_active: true,
      permissions: { trade: true, withdraw: false },
      created_at: "2026-01-01T00:00:00Z",
      last_used_at: "2026-03-25T12:00:00Z",
    });
    expect(result.success).toBe(true);
  });

  it("accepts null last_used_at", () => {
    const result = ExchangeAccountSchema.safeParse({
      id: "acc-123",
      exchange_name: "binance",
      account_name: "Main",
      is_active: false,
      permissions: {},
      created_at: "2026-01-01T00:00:00Z",
      last_used_at: null,
    });
    expect(result.success).toBe(true);
  });

  it("rejects missing id", () => {
    const result = ExchangeAccountSchema.safeParse({
      exchange_name: "binance",
      account_name: "Main",
      is_active: true,
      permissions: {},
      created_at: "2026-01-01",
      last_used_at: null,
    });
    expect(result.success).toBe(false);
  });
});

// --- ExchangeAccountsResponseSchema ---

describe("ExchangeAccountsResponseSchema", () => {
  const validAccount = {
    id: "acc-1",
    exchange_name: "binance",
    account_name: "Main",
    is_active: true,
    permissions: {},
    created_at: "2026-01-01",
    last_used_at: null,
  };

  it("accepts bare array format", () => {
    const result = ExchangeAccountsResponseSchema.safeParse([validAccount]);
    expect(result.success).toBe(true);
  });

  it("accepts object with data array", () => {
    const result = ExchangeAccountsResponseSchema.safeParse({
      data: [validAccount],
    });
    expect(result.success).toBe(true);
  });

  it("accepts object with accounts array", () => {
    const result = ExchangeAccountsResponseSchema.safeParse({
      accounts: [validAccount],
    });
    expect(result.success).toBe(true);
  });

  it("accepts empty array", () => {
    const result = ExchangeAccountsResponseSchema.safeParse([]);
    expect(result.success).toBe(true);
  });
});

// --- AddExchangeAccountResponseSchema ---

describe("AddExchangeAccountResponseSchema", () => {
  it("accepts success response with data", () => {
    const result = AddExchangeAccountResponseSchema.safeParse({
      success: true,
      data: {
        id: "acc-new",
        exchange_name: "woo",
        account_name: "WOO Main",
        is_active: true,
        permissions: {},
        created_at: "2026-03-25",
        last_used_at: null,
      },
    });
    expect(result.success).toBe(true);
  });

  it("accepts error response", () => {
    const result = AddExchangeAccountResponseSchema.safeParse({
      success: false,
      error: "Invalid credentials",
    });
    expect(result.success).toBe(true);
  });

  it("accepts empty object (all optional)", () => {
    const result = AddExchangeAccountResponseSchema.safeParse({});
    expect(result.success).toBe(true);
  });
});

// --- TestConnectionResultSchema ---

describe("TestConnectionResultSchema", () => {
  it("accepts valid test result", () => {
    const result = TestConnectionResultSchema.safeParse({
      account_id: "acc-123",
      exchange_name: "binance",
      status: "connected",
      message: "Connection successful",
      tested_at: "2026-03-25T12:00:00Z",
      latency_ms: 42,
    });
    expect(result.success).toBe(true);
  });

  it("accepts null latency_ms", () => {
    const result = TestConnectionResultSchema.safeParse({
      account_id: "acc-123",
      exchange_name: "binance",
      status: "error",
      message: "Connection timed out",
      tested_at: "2026-03-25T12:00:00Z",
      latency_ms: null,
    });
    expect(result.success).toBe(true);
  });
});

// --- ExchangeBalanceApiResponseSchema ---

describe("ExchangeBalanceApiResponseSchema", () => {
  it("accepts valid balance response", () => {
    const result = ExchangeBalanceApiResponseSchema.safeParse({
      account_id: "acc-123",
      exchange_name: "binance",
      balances: [
        { asset: "USDT", total: "10000.50", free: "9500.00", used: "500.50" },
      ],
      fetched_at: "2026-03-25T12:00:00Z",
    });
    expect(result.success).toBe(true);
  });

  it("accepts empty balances array", () => {
    const result = ExchangeBalanceApiResponseSchema.safeParse({
      account_id: "acc-123",
      exchange_name: "binance",
      balances: [],
      fetched_at: "2026-03-25",
    });
    expect(result.success).toBe(true);
  });

  it("rejects missing balances", () => {
    const result = ExchangeBalanceApiResponseSchema.safeParse({
      account_id: "acc-123",
      exchange_name: "binance",
      fetched_at: "2026-03-25",
    });
    expect(result.success).toBe(false);
  });
});

// --- ExchangePositionSchema ---

describe("ExchangePositionSchema", () => {
  it("accepts valid position", () => {
    const result = ExchangePositionSchema.safeParse({
      symbol: "BTC/USDT:USDT",
      side: "long",
      contracts: "0.5",
      entry_price: "65000.00",
      unrealized_pnl: "250.00",
    });
    expect(result.success).toBe(true);
  });

  it("rejects missing symbol", () => {
    const result = ExchangePositionSchema.safeParse({
      side: "long",
      contracts: "0.5",
      entry_price: "65000.00",
      unrealized_pnl: "250.00",
    });
    expect(result.success).toBe(false);
  });
});

// --- ExchangeOpenOrderSchema ---

describe("ExchangeOpenOrderSchema", () => {
  it("accepts valid open order", () => {
    const result = ExchangeOpenOrderSchema.safeParse({
      id: "order-1",
      symbol: "BTC/USDT:USDT",
      side: "buy",
      type: "limit",
      price: "64000.00",
      amount: "0.1",
    });
    expect(result.success).toBe(true);
  });

  it("accepts null price and stop_price", () => {
    const result = ExchangeOpenOrderSchema.safeParse({
      id: "order-2",
      symbol: "BTC/USDT:USDT",
      side: "sell",
      type: "market",
      price: null,
      stop_price: null,
      amount: "0.1",
    });
    expect(result.success).toBe(true);
  });

  it("accepts missing optional price fields", () => {
    const result = ExchangeOpenOrderSchema.safeParse({
      id: "order-3",
      symbol: "ETH/USDT:USDT",
      side: "buy",
      type: "stop",
      amount: "1.0",
    });
    expect(result.success).toBe(true);
  });
});

// --- ExchangePositionsApiResponseSchema ---

describe("ExchangePositionsApiResponseSchema", () => {
  it("accepts valid positions response", () => {
    const result = ExchangePositionsApiResponseSchema.safeParse({
      account_id: "acc-123",
      exchange_name: "binance",
      positions: [
        {
          symbol: "BTC/USDT:USDT",
          side: "long",
          contracts: "0.5",
          entry_price: "65000",
          unrealized_pnl: "100",
        },
      ],
      open_orders: [
        {
          id: "o1",
          symbol: "BTC/USDT:USDT",
          side: "sell",
          type: "limit",
          price: "70000",
          amount: "0.5",
        },
      ],
      fetched_at: "2026-03-25T12:00:00Z",
    });
    expect(result.success).toBe(true);
  });

  it("accepts empty positions and orders", () => {
    const result = ExchangePositionsApiResponseSchema.safeParse({
      account_id: "acc-123",
      exchange_name: "binance",
      positions: [],
      open_orders: [],
      fetched_at: "2026-03-25",
    });
    expect(result.success).toBe(true);
  });
});

// --- SidecarHealthResponseSchema ---

describe("SidecarHealthResponseSchema", () => {
  it("accepts status response", () => {
    const result = SidecarHealthResponseSchema.safeParse({ status: "ok" });
    expect(result.success).toBe(true);
  });

  it("accepts empty object (status optional)", () => {
    const result = SidecarHealthResponseSchema.safeParse({});
    expect(result.success).toBe(true);
  });
});

// --- WebSocketMessageSchema ---

describe("WebSocketMessageSchema", () => {
  it("accepts message with stream and data", () => {
    const result = WebSocketMessageSchema.safeParse({
      stream: "order.user-123",
      data: { type: "fill", price: "65000" },
    });
    expect(result.success).toBe(true);
  });

  it("accepts empty object (all optional)", () => {
    const result = WebSocketMessageSchema.safeParse({});
    expect(result.success).toBe(true);
  });
});

// --- SidecarStreamDataSchema ---

describe("SidecarStreamDataSchema", () => {
  it("accepts status data", () => {
    const result = SidecarStreamDataSchema.safeParse({ status: "connected" });
    expect(result.success).toBe(true);
  });

  it("accepts empty object", () => {
    const result = SidecarStreamDataSchema.safeParse({});
    expect(result.success).toBe(true);
  });
});

// --- RuntimeMessageSchema (discriminated union) ---

describe("RuntimeMessageSchema", () => {
  it("accepts GET_SETTINGS message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "GET_SETTINGS" });
    expect(result.success).toBe(true);
  });

  it("accepts EXECUTE_TRADE with valid payload", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "EXECUTE_TRADE",
      payload: {
        symbol: "BTC_USDT",
        side: "LONG",
        entry: 65000,
        stop: 63000,
        target: 70000,
        timeframe: "15m",
        management: {
          risk_percent: 1.0,
          break_even_at: 50,
          trailing_stop: { enabled: false, distance_percent: 25 },
          partial_tp: { enabled: false, close_percent: 50 },
        },
      },
    });
    expect(result.success).toBe(true);
  });

  it("rejects EXECUTE_TRADE without payload", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "EXECUTE_TRADE" });
    expect(result.success).toBe(false);
  });

  it("accepts PAIR with 6-char code", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "PAIR", code: "ABC123" });
    expect(result.success).toBe(true);
  });

  it("rejects PAIR with wrong-length code", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "PAIR", code: "AB" });
    expect(result.success).toBe(false);
  });

  it("rejects PAIR without code", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "PAIR" });
    expect(result.success).toBe(false);
  });

  it("accepts LOGOUT message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "LOGOUT" });
    expect(result.success).toBe(true);
  });

  it("accepts AUTH_STATUS message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "AUTH_STATUS" });
    expect(result.success).toBe(true);
  });

  it("accepts REFRESH_TOKEN message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "REFRESH_TOKEN" });
    expect(result.success).toBe(true);
  });

  it("accepts WS_STATUS message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "WS_STATUS" });
    expect(result.success).toBe(true);
  });

  it("accepts WS_RECONNECT message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "WS_RECONNECT" });
    expect(result.success).toBe(true);
  });

  it("accepts LIST_TRADES message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "LIST_TRADES" });
    expect(result.success).toBe(true);
  });

  it("accepts CANCEL_TRADE with tradeId", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "CANCEL_TRADE",
      tradeId: "trade-abc-123",
    });
    expect(result.success).toBe(true);
  });

  it("rejects CANCEL_TRADE without tradeId", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "CANCEL_TRADE" });
    expect(result.success).toBe(false);
  });

  it("accepts GET_BALANCE message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "GET_BALANCE" });
    expect(result.success).toBe(true);
  });

  it("accepts LIST_EXCHANGES message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "LIST_EXCHANGES" });
    expect(result.success).toBe(true);
  });

  it("accepts LIST_EXCHANGE_ACCOUNTS message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "LIST_EXCHANGE_ACCOUNTS" });
    expect(result.success).toBe(true);
  });

  it("accepts ADD_EXCHANGE_ACCOUNT with payload", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "ADD_EXCHANGE_ACCOUNT",
      payload: {
        exchange_name: "binance",
        account_name: "Main",
        api_key: "key-abc",
        secret: "secret-xyz",
      },
    });
    expect(result.success).toBe(true);
  });

  it("accepts ADD_EXCHANGE_ACCOUNT with optional passphrase", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "ADD_EXCHANGE_ACCOUNT",
      payload: {
        exchange_name: "okx",
        api_key: "key",
        secret: "secret",
        passphrase: "pass",
      },
    });
    expect(result.success).toBe(true);
  });

  it("rejects ADD_EXCHANGE_ACCOUNT without required api_key", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "ADD_EXCHANGE_ACCOUNT",
      payload: {
        exchange_name: "binance",
        secret: "secret",
      },
    });
    expect(result.success).toBe(false);
  });

  it("accepts DELETE_EXCHANGE_ACCOUNT with accountId", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "DELETE_EXCHANGE_ACCOUNT",
      accountId: "acc-123",
    });
    expect(result.success).toBe(true);
  });

  it("accepts TEST_EXCHANGE_CONNECTION with accountId", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "TEST_EXCHANGE_CONNECTION",
      accountId: "acc-123",
    });
    expect(result.success).toBe(true);
  });

  it("accepts GET_ACTIVE_EXCHANGE message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "GET_ACTIVE_EXCHANGE" });
    expect(result.success).toBe(true);
  });

  it("accepts SET_ACTIVE_EXCHANGE with exchangeId", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "SET_ACTIVE_EXCHANGE",
      exchangeId: "exc-123",
    });
    expect(result.success).toBe(true);
  });

  it("accepts SET_ACTIVE_EXCHANGE with null exchangeId", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "SET_ACTIVE_EXCHANGE",
      exchangeId: null,
    });
    expect(result.success).toBe(true);
  });

  it("accepts SIDECAR_STATUS message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "SIDECAR_STATUS" });
    expect(result.success).toBe(true);
  });

  it("accepts EXCHANGE_POSITIONS message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "EXCHANGE_POSITIONS" });
    expect(result.success).toBe(true);
  });

  it("accepts CLOSE_EXCHANGE_POSITION with all fields", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "CLOSE_EXCHANGE_POSITION",
      symbol: "BTC/USDT:USDT",
      side: "long",
      contracts: "0.5",
    });
    expect(result.success).toBe(true);
  });

  it("accepts CLOSE_EXCHANGE_POSITION with short side", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "CLOSE_EXCHANGE_POSITION",
      symbol: "ETH/USDT:USDT",
      side: "short",
      contracts: "1.0",
    });
    expect(result.success).toBe(true);
  });

  it("rejects CLOSE_EXCHANGE_POSITION with invalid side", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "CLOSE_EXCHANGE_POSITION",
      symbol: "BTC/USDT:USDT",
      side: "LONG",
      contracts: "0.5",
    });
    expect(result.success).toBe(false);
  });

  it("accepts CLEANUP_TRADES message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "CLEANUP_TRADES" });
    expect(result.success).toBe(true);
  });

  it("accepts GET_EXCHANGE_MODE message", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "GET_EXCHANGE_MODE" });
    expect(result.success).toBe(true);
  });

  it("accepts SET_EXCHANGE_MODE with cex", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "SET_EXCHANGE_MODE",
      mode: "cex",
    });
    expect(result.success).toBe(true);
  });

  it("accepts SET_EXCHANGE_MODE with dex", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "SET_EXCHANGE_MODE",
      mode: "dex",
    });
    expect(result.success).toBe(true);
  });

  it("rejects SET_EXCHANGE_MODE with invalid mode", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "SET_EXCHANGE_MODE",
      mode: "hybrid",
    });
    expect(result.success).toBe(false);
  });

  it("accepts ACCOUNT_LINKED with optional account", () => {
    const result = RuntimeMessageSchema.safeParse({
      type: "ACCOUNT_LINKED",
      account: {
        id: "acc-1",
        exchange_name: "binance",
      },
    });
    expect(result.success).toBe(true);
  });

  it("accepts ACCOUNT_LINKED without account", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "ACCOUNT_LINKED" });
    expect(result.success).toBe(true);
  });

  it("rejects unknown message type", () => {
    const result = RuntimeMessageSchema.safeParse({ type: "UNKNOWN_TYPE" });
    expect(result.success).toBe(false);
  });

  it("rejects message without type field", () => {
    const result = RuntimeMessageSchema.safeParse({ data: "test" });
    expect(result.success).toBe(false);
  });

  it("rejects non-object input", () => {
    const result = RuntimeMessageSchema.safeParse("GET_SETTINGS");
    expect(result.success).toBe(false);
  });

  it("rejects null input", () => {
    const result = RuntimeMessageSchema.safeParse(null);
    expect(result.success).toBe(false);
  });
});
