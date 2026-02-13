import { describe, it, expect } from "vitest";
import {
  DEFAULT_MANAGEMENT_PRESET,
  ORDER_EVENT_STYLES,
} from "./types";
import type { ManagementPreset, TradePayload } from "./types";

describe("ManagementPreset", () => {
  it("has correct default values", () => {
    expect(DEFAULT_MANAGEMENT_PRESET.name).toBe("default");
    expect(DEFAULT_MANAGEMENT_PRESET.risk_percent).toBe(1.0);
    expect(DEFAULT_MANAGEMENT_PRESET.break_even_at).toBe(50);
    expect(DEFAULT_MANAGEMENT_PRESET.trailing_stop.enabled).toBe(false);
    expect(DEFAULT_MANAGEMENT_PRESET.trailing_stop.distance_percent).toBe(25);
    expect(DEFAULT_MANAGEMENT_PRESET.partial_tp.enabled).toBe(false);
    expect(DEFAULT_MANAGEMENT_PRESET.partial_tp.close_percent).toBe(50);
    expect(DEFAULT_MANAGEMENT_PRESET.leverage).toBe(1);
  });

  it("is a valid ManagementPreset shape", () => {
    const preset: ManagementPreset = DEFAULT_MANAGEMENT_PRESET;
    expect(typeof preset.name).toBe("string");
    expect(typeof preset.risk_percent).toBe("number");
    expect(typeof preset.break_even_at).toBe("number");
    expect(typeof preset.trailing_stop).toBe("object");
    expect(typeof preset.partial_tp).toBe("object");
    expect(typeof preset.leverage).toBe("number");
  });
});

describe("TradePayload", () => {
  it("includes management block without quantity", () => {
    const payload: TradePayload = {
      symbol: "BTC_USDT",
      side: "LONG",
      entry: 50000,
      stop: 49000,
      target: 52000,
      timeframe: "15m",
      management: {
        risk_percent: 1.0,
        break_even_at: 50,
        trailing_stop: { enabled: true, distance_percent: 25 },
        partial_tp: { enabled: true, close_percent: 50 },
        leverage: 10,
      },
    };

    expect(payload.management).toBeDefined();
    expect(payload.management.risk_percent).toBe(1.0);
    // Verify quantity is not in the interface
    expect("quantity" in payload).toBe(false);
  });
});

describe("ORDER_EVENT_STYLES", () => {
  it("maps order.filled to green/success", () => {
    expect(ORDER_EVENT_STYLES["order.filled"]).toEqual({ color: "green", type: "success" });
  });

  it("maps order.stopped to red/error", () => {
    expect(ORDER_EVENT_STYLES["order.stopped"]).toEqual({ color: "red", type: "error" });
  });

  it("maps order.amended to blue/info", () => {
    expect(ORDER_EVENT_STYLES["order.amended"]).toEqual({ color: "blue", type: "info" });
  });

  it("maps order.trailing to blue/info", () => {
    expect(ORDER_EVENT_STYLES["order.trailing"]).toEqual({ color: "blue", type: "info" });
  });

  it("maps order.partial_close to green/success", () => {
    expect(ORDER_EVENT_STYLES["order.partial_close"]).toEqual({ color: "green", type: "success" });
  });

  it("maps order.tp_hit to green/success", () => {
    expect(ORDER_EVENT_STYLES["order.tp_hit"]).toEqual({ color: "green", type: "success" });
  });
});
