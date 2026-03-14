export interface Settings {
  backendUrl: string;
  wsUrl: string;
}

export interface AuthTokens {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

export interface LoginResponse {
  user: { id: string; email: string };
  tokens: AuthTokens;
}

export interface ManagementPreset {
  name: string;
  risk_percent: number;
  break_even_enabled: boolean;
  break_even_at: number;
  leverage: number;
  trailing_stop: {
    enabled: boolean;
    distance_percent: number;
  };
  partial_tp: {
    enabled: boolean;
    close_percent: number;
  };
}

export const DEFAULT_MANAGEMENT_PRESET: ManagementPreset = {
  name: "default",
  risk_percent: 1.0,
  break_even_enabled: true,
  break_even_at: 50,
  leverage: 1,
  trailing_stop: { enabled: false, distance_percent: 25 },
  partial_tp: { enabled: false, close_percent: 50 },
};

export interface TradePayload {
  symbol: string;
  side: "LONG" | "SHORT";
  entry: number;
  stop: number;
  target: number;
  timeframe: string;
  exchange_account_id?: string;
  management: {
    risk_percent: number;
    break_even_enabled: boolean;
    break_even_at: number;
    leverage: number;
    trailing_stop: { enabled: boolean; distance_percent: number };
    partial_tp: { enabled: boolean; close_percent: number };
  };
}

export interface BalanceResponse {
  asset: string;
  available: string;
  locked: string;
}

export interface BackendResponse {
  success: boolean;
  data?: unknown;
  error?: string | null;
  warnings?: string[];
}

export interface TakeProfitTargetResponse {
  price: string;
  percent_to_close: string;
  order_id: string | null;
  filled: boolean;
}

export interface TradeGroupResponse {
  id: string;
  symbol: string;
  entry_order_id: string;
  entry_price: string | null;
  entry_quantity: string;
  stop_loss_price: string | null;
  stop_loss_order_id: string | null;
  take_profit_targets: TakeProfitTargetResponse[];
  status: string;
  break_even_enabled: boolean;
  break_even_triggered: boolean;
  trailing_stop_enabled?: boolean;
  created_at: string;
  updated_at: string;
}

export interface ScraperHealthRecord {
  timestamp: number;
  strategyUsed: number | null; // 0-5 for strategy index, null = all failed
  success: boolean;
}

export interface ChartApiHealth {
  available: boolean;
  hasActiveChart: boolean;
  hasGetAllShapes: boolean;
  hasGetShapeById: boolean;
}

// --- Exchange Account Types (EXT-15) ---

export interface ExchangeInfo {
  id: string;
  name: string;
  type: string;
  description: string;
  supported_features: string[];
  required_credentials: string[];
  optional_credentials: string[];
}

export interface ExchangeAccount {
  id: string;
  exchange_name: string;
  account_name: string;
  is_active: boolean;
  permissions: Record<string, unknown>;
  created_at: string;
  last_used_at: string | null;
}

export interface AddExchangeAccountPayload {
  exchange_name: string;
  account_name?: string;
  api_key: string;
  secret: string;
  passphrase?: string;
}

export interface TestConnectionResult {
  account_id: string;
  exchange_name: string;
  status: string;
  message: string;
  tested_at: string;
  latency_ms: number | null;
}

// --- EXT-17: Live Balance Types ---

export interface LiveBalanceResponse {
  exchange_name?: string;
  balances: BalanceResponse[];
}

// --- Exchange Positions Types ---

export interface ExchangePosition {
  symbol: string;
  side: string;
  contracts: string;
  entry_price: string;
  unrealized_pnl: string;
}

export interface ExchangeOpenOrder {
  id: string;
  symbol: string;
  side: string;
  type: string;
  price: string | null;
  stop_price: string | null;
  amount: string;
}

export interface ExchangePositionsResponse {
  account_id: string;
  exchange_name: string;
  positions: ExchangePosition[];
  open_orders: ExchangeOpenOrder[];
  fetched_at: string;
}

export type WsState = "disconnected" | "connecting" | "connected";

export type ToastType = "success" | "error" | "info";

export type OrderEventType =
  | "order.filled"
  | "order.amended"
  | "order.trailing"
  | "order.partial_close"
  | "order.stopped"
  | "order.tp_hit"
  | "order.break_even"
  | "order.trailing_moved"
  | "order.partial_tp";

export const ORDER_EVENT_STYLES: Record<string, { color: string; type: ToastType }> = {
  "order.filled": { color: "green", type: "success" },
  "order.amended": { color: "blue", type: "info" },
  "order.trailing": { color: "blue", type: "info" },
  "order.partial_close": { color: "green", type: "success" },
  "order.stopped": { color: "red", type: "error" },
  "order.tp_hit": { color: "green", type: "success" },
  "order.break_even": { color: "blue", type: "info" },
  "order.trailing_moved": { color: "blue", type: "info" },
  "order.partial_tp": { color: "green", type: "success" },
};
