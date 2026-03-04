import { z } from "zod";

export const SettingsSchema = z.object({
  backendUrl: z.string().url(),
  wsUrl: z.string().url(),
});

export const StoredSettingsSchema = z.object({
  backendUrl: z.string().url().optional(),
  wsUrl: z.string().url().optional(),
});

export const AuthTokensSchema = z.object({
  access_token: z.string().min(1),
  refresh_token: z.string().min(1),
  expires_in: z.number().int(),
});

export const StoredTokensSchema = z.object({
  accessToken: z.string(),
  refreshToken: z.string(),
  tokenExpiry: z.number().optional(),
});

export const RefreshResponseSchema = z.object({
  tokens: AuthTokensSchema,
});

export const ActiveExchangeStorageSchema = z.object({
  activeExchangeId: z.string().optional(),
});

export const LoginResponseSchema = z.object({
  user: z.object({
    id: z.string().min(1),
    email: z.string().email(),
  }),
  tokens: AuthTokensSchema,
});

export const JwtEmailPayloadSchema = z.object({
  email: z.string().email().optional(),
});

export const JwtSubPayloadSchema = z.object({
  sub: z.string().min(1).optional(),
});

export const TradePayloadSchema = z.object({
  symbol: z.string().min(1),
  side: z.enum(["LONG", "SHORT"]),
  entry: z.number().positive(),
  stop: z.number().positive(),
  target: z.number().positive(),
  timeframe: z.string().min(1),
  exchange_account_id: z.string().min(1).optional(),
  management: z.object({
    risk_percent: z.number().min(0.1).max(100),
    break_even_at: z.number().min(0).max(100),
    leverage: z.number().min(1).max(100).optional(),
    trailing_stop: z.object({
      enabled: z.boolean(),
      distance_percent: z.number().min(0),
    }),
    partial_tp: z.object({
      enabled: z.boolean(),
      close_percent: z.number().min(0).max(100),
    }),
  }),
});

export const BackendResponseSchema = z.object({
  success: z.boolean(),
  data: z.unknown().optional(),
  error: z.string().nullable().optional(),
});

export const ErrorResponseSchema = z.object({
  error: z.string().optional(),
  message: z.string().optional(),
});

export const TradeGroupResponseSchema = z.object({
  id: z.string(),
  symbol: z.string(),
  entry_order_id: z.string(),
  entry_price: z.string().nullable(),
  entry_quantity: z.string(),
  stop_loss_price: z.string().nullable(),
  stop_loss_order_id: z.string().nullable(),
  take_profit_targets: z.array(z.object({
    price: z.string(),
    percent_to_close: z.string(),
    order_id: z.string().nullable(),
    filled: z.boolean(),
  })),
  status: z.string(),
  break_even_enabled: z.boolean(),
  break_even_triggered: z.boolean(),
});

export const TradeListResponseSchema = z.object({
  success: z.boolean(),
  data: z.array(TradeGroupResponseSchema).optional(),
  error: z.string().optional(),
});

export const ExchangeInfoSchema = z.object({
  id: z.string(),
  name: z.string(),
  type: z.string(),
  description: z.string(),
  supported_features: z.array(z.string()),
  required_credentials: z.array(z.string()),
  optional_credentials: z.array(z.string()),
});

export const ListExchangesResponseSchema = z.object({
  exchanges: z.array(ExchangeInfoSchema).optional(),
  error: z.string().optional(),
});

export const ExchangeAccountSchema = z.object({
  id: z.string(),
  exchange_name: z.string(),
  account_name: z.string(),
  is_active: z.boolean(),
  permissions: z.record(z.string(), z.unknown()),
  created_at: z.string(),
  last_used_at: z.string().nullable(),
});

export const ExchangeAccountsResponseSchema = z.union([
  z.array(ExchangeAccountSchema),
  z.object({
    data: z.array(ExchangeAccountSchema).optional(),
    accounts: z.array(ExchangeAccountSchema).optional(),
  }),
]);

export const AddExchangeAccountResponseSchema = z.object({
  success: z.boolean().optional(),
  data: ExchangeAccountSchema.optional(),
  error: z.string().optional(),
});

export const TestConnectionResultSchema = z.object({
  account_id: z.string(),
  exchange_name: z.string(),
  status: z.string(),
  message: z.string(),
  tested_at: z.string(),
  latency_ms: z.number().nullable(),
});

export const ExchangeBalanceApiResponseSchema = z.object({
  account_id: z.string(),
  exchange_name: z.string(),
  balances: z.array(z.object({
    asset: z.string(),
    total: z.string(),
    free: z.string(),
    used: z.string(),
  })),
  fetched_at: z.string(),
});

export const SidecarHealthResponseSchema = z.object({
  status: z.string().optional(),
});

export const WebSocketMessageSchema = z.object({
  stream: z.string().optional(),
  data: z.unknown().optional(),
});

export const SidecarStreamDataSchema = z.object({
  status: z.string().optional(),
});

export const RuntimeMessageSchema = z.discriminatedUnion("type", [
  z.object({ type: z.literal("GET_SETTINGS") }),
  z.object({ type: z.literal("EXECUTE_TRADE"), payload: TradePayloadSchema }),
  z.object({ type: z.literal("LOGIN"), email: z.string(), password: z.string() }),
  z.object({ type: z.literal("REGISTER"), email: z.string(), password: z.string() }),
  z.object({ type: z.literal("LOGOUT") }),
  z.object({ type: z.literal("AUTH_STATUS") }),
  z.object({ type: z.literal("REFRESH_TOKEN") }),
  z.object({ type: z.literal("WS_STATUS") }),
  z.object({ type: z.literal("WS_RECONNECT") }),
  z.object({ type: z.literal("LIST_TRADES") }),
  z.object({ type: z.literal("CANCEL_TRADE"), tradeId: z.string() }),
  z.object({ type: z.literal("GET_BALANCE") }),
  z.object({ type: z.literal("LIST_EXCHANGES") }),
  z.object({ type: z.literal("LIST_EXCHANGE_ACCOUNTS") }),
  z.object({
    type: z.literal("ADD_EXCHANGE_ACCOUNT"),
    payload: z.object({
      exchange_name: z.string(),
      account_name: z.string().optional(),
      api_key: z.string(),
      secret: z.string(),
      passphrase: z.string().optional(),
    }),
  }),
  z.object({ type: z.literal("DELETE_EXCHANGE_ACCOUNT"), accountId: z.string() }),
  z.object({ type: z.literal("TEST_EXCHANGE_CONNECTION"), accountId: z.string() }),
  z.object({ type: z.literal("GET_ACTIVE_EXCHANGE") }),
  z.object({ type: z.literal("SET_ACTIVE_EXCHANGE"), exchangeId: z.string().nullable() }),
  z.object({ type: z.literal("SIDECAR_STATUS") }),
  z.object({ type: z.literal("TOKEN_SYNCED_FROM_WEB") }),
]);
