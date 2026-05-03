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

export const PairResponseSchema = z.object({
  user: z.object({
    id: z.string().min(1),
    wallet_address: z.string().min(1),
  }),
  tokens: AuthTokensSchema,
});

export const JwtWalletPayloadSchema = z.object({
  wallet_address: z.string().min(1).optional(),
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
  setup_tag: z.string().trim().max(48).nullable().optional(),
  management: z.object({
    risk_percent: z.number().min(0.1).max(100),
    break_even_enabled: z.boolean().optional().default(true),
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
  error_code: z.string().optional(),
  warnings: z.array(z.string()).optional(),
});

export const ErrorResponseSchema = z.object({
  error: z.string().optional(),
  message: z.string().optional(),
  error_code: z.string().optional(),
});

const DecimalLikeStringSchema = z.union([z.string(), z.number()]).transform((v) => String(v));

export const TradeGroupResponseSchema = z.object({
  id: z.string(),
  symbol: z.string(),
  entry_order_id: z.string(),
  entry_price: DecimalLikeStringSchema.nullable().optional().default(null),
  entry_quantity: DecimalLikeStringSchema,
  stop_loss_price: DecimalLikeStringSchema.nullable().optional().default(null),
  stop_loss_order_id: z.string().nullable().optional().default(null),
  take_profit_targets: z.array(z.object({
    price: DecimalLikeStringSchema,
    percent_to_close: DecimalLikeStringSchema,
    order_id: z.string().nullable().optional().default(null),
    filled: z.boolean(),
  })).optional().default([]),
  status: z.string(),
  break_even_enabled: z.boolean().optional().default(false),
  break_even_triggered: z.boolean().optional().default(false),
});

export const TradeListResponseSchema = z.object({
  success: z.boolean(),
  data: z.array(TradeGroupResponseSchema).nullable().optional(),
  error: z.string().nullable().optional(),
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

export const ExchangePositionSchema = z.object({
  symbol: z.string(),
  side: z.string(),
  contracts: z.string(),
  entry_price: z.string(),
  unrealized_pnl: z.string(),
});

export const ExchangeOpenOrderSchema = z.object({
  id: z.string(),
  symbol: z.string(),
  side: z.string(),
  type: z.string(),
  price: z.string().nullable().optional(),
  stop_price: z.string().nullable().optional(),
  amount: z.string(),
});

export const ExchangePositionsApiResponseSchema = z.object({
  account_id: z.string(),
  exchange_name: z.string(),
  positions: z.array(ExchangePositionSchema),
  open_orders: z.array(ExchangeOpenOrderSchema),
  fetched_at: z.string(),
});

export const SidecarHealthResponseSchema = z.object({
  status: z.string().optional(),
});

export const SetupTagEntrySchema = z.object({
  name: z.string(),
  last_used: z.string(),
  uses: z.number().int().nonnegative(),
});

export const SetupTagsResponseSchema = z.array(SetupTagEntrySchema);

export const UserSettingsSchema = z.object({
  dynamic_risk_enabled: z.boolean(),
  dynamic_risk_unlocked_at: z.string().nullable(),
});

export const UserSettingsResponseSchema = z.object({
  settings: UserSettingsSchema,
  unlocked: z.boolean(),
  tagged_trade_count: z.number().int().nonnegative(),
});

// QNT-01b: SizingPreview (POST /api/v1/trades/preview response)
// Backend serializes Decimal as string; coerce to number for frontend math.
const DecimalNumberSchema = z.union([z.string(), z.number()]).transform((v) => Number(v));

const CalibratedReasoningSchema = z.object({
  kind: z.literal("calibrated"),
  n_setup: z.number().int().nonnegative(),
  p_eff: DecimalNumberSchema,
  avg_r_win: DecimalNumberSchema,
  avg_r_loss: DecimalNumberSchema,
});

const UntaggedReasoningSchema = z.object({
  kind: z.literal("untagged"),
});

const NegativeEdgeReasoningSchema = z.object({
  kind: z.literal("negative_edge"),
  quarter_kelly: DecimalNumberSchema,
});

const FixedModeReasoningSchema = z.object({
  kind: z.literal("fixed_mode"),
});

export const SizingPreviewSchema = z.object({
  baseline_risk_pct: DecimalNumberSchema,
  effective_risk_pct: DecimalNumberSchema,
  edge_multiplier: DecimalNumberSchema,
  reasoning: z.discriminatedUnion("kind", [
    CalibratedReasoningSchema,
    UntaggedReasoningSchema,
    NegativeEdgeReasoningSchema,
    FixedModeReasoningSchema,
  ]),
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
  z.object({ type: z.literal("PAIR"), code: z.string().length(6) }),
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
  z.object({ type: z.literal("EXCHANGE_POSITIONS") }),
  z.object({
    type: z.literal("CLOSE_EXCHANGE_POSITION"),
    symbol: z.string(),
    side: z.enum(["long", "short"]),
    contracts: z.string(),
  }),
  z.object({ type: z.literal("CLEANUP_TRADES") }),
  z.object({ type: z.literal("GET_EXCHANGE_MODE") }),
  z.object({ type: z.literal("SET_EXCHANGE_MODE"), mode: z.enum(["cex", "dex"]) }),
  z.object({
    type: z.literal("ACCOUNT_LINKED"),
    account: z.object({
      id: z.string().optional(),
      exchange_name: z.string().optional(),
    }).optional(),
  }),
  z.object({
    type: z.literal("GET_SETUP_TAGS"),
    limit: z.number().int().min(1).max(100).optional(),
  }),
  z.object({ type: z.literal("GET_USER_SETTINGS") }),
  z.object({
    type: z.literal("PATCH_USER_SETTINGS"),
    dynamic_risk_enabled: z.boolean(),
  }),
  z.object({
    type: z.literal("PREVIEW_TRADE_SIZING"),
    payload: TradePayloadSchema,
  }),
  // Emitted by the desk.testudo.vip content script when the web app's
  // active wallet changes (including logout → wallet_address: null).
  // Extension clears its paired JWT if the new web wallet differs.
  z.object({
    type: z.literal("WEB_WALLET_CHANGED"),
    wallet_address: z.string().nullable(),
  }),
]);
