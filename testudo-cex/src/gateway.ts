/** @anchor api:cex:gateway
 * @tags api */

import { createExchange } from "safe-cex";
import type { BaseExchange } from "safe-cex/dist/exchanges/base";
import type {
  ExchangeName,
  ExchangeOptions,
  OrderFillEvent,
  LogSeverity,
} from "safe-cex/dist/types";
import crypto from "crypto";

export interface Credentials {
  key: string;
  secret: string;
  applicationId?: string;
  passphrase?: string;
}

export type OnFillCallback = (fill: OrderFillEvent) => void;

export class ExchangeGateway {
  private instances = new Map<string, BaseExchange>();

  cacheKey(exchangeId: string, apiKey: string, sandbox: boolean): string {
    return crypto
      .createHash("sha256")
      .update(`${exchangeId}:${apiKey}:${sandbox}`)
      .digest("hex")
      .slice(0, 16);
  }

  async getOrCreate(
    exchangeId: ExchangeName,
    credentials: Credentials,
    sandbox: boolean,
    onFill: OnFillCallback
  ): Promise<BaseExchange> {
    const key = this.cacheKey(exchangeId, credentials.key, sandbox);

    const existing = this.instances.get(key);
    if (existing) return existing;

    const opts: ExchangeOptions = {
      key: credentials.key,
      secret: credentials.secret,
      applicationId: credentials.applicationId,
      passphrase: credentials.passphrase,
      testnet: sandbox,
    };

    const exchange = createExchange(exchangeId, opts);

    exchange.on("fill", onFill);
    exchange.on("error", (err: string) =>
      console.error(`[${exchangeId}] error:`, err)
    );
    exchange.on("log", (msg: string, severity: LogSeverity) =>
      console.log(`[${exchangeId}] ${severity}:`, msg)
    );

    try {
      await exchange.start();
    } catch (err: any) {
      // Extract meaningful message before re-throwing
      const detail = err?.response?.data?.msg || err?.response?.data?.message || err?.message || String(err);
      console.error(`[${exchangeId}] start() failed: ${detail}`);
      // Clean up — don't leave a broken instance
      try { exchange.dispose(); } catch {}
      throw err;
    }

    this.instances.set(key, exchange);
    return exchange;
  }

  async dispose(key: string): Promise<void> {
    const instance = this.instances.get(key);
    if (instance) {
      instance.dispose();
      this.instances.delete(key);
    }
  }

  async disposeAll(): Promise<void> {
    for (const [key] of this.instances) {
      await this.dispose(key);
    }
  }

  get size(): number {
    return this.instances.size;
  }
}
