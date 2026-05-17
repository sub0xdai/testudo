import { vi } from "vitest";

const storage: Record<string, unknown> = {};

const browser = {
  storage: {
    local: {
      get: vi.fn(async (keys: string[]) => {
        const result: Record<string, unknown> = {};
        for (const key of keys) {
          if (key in storage) result[key] = storage[key];
        }
        return result;
      }),
      set: vi.fn(async (items: Record<string, unknown>) => {
        Object.assign(storage, items);
      }),
      remove: vi.fn(async (keys: string[]) => {
        for (const key of keys) delete storage[key];
      }),
    },
    onChanged: {
      addListener: vi.fn(),
    },
  },
  runtime: {
    sendMessage: vi.fn(async () => undefined),
    onMessage: {
      addListener: vi.fn(),
    },
    onInstalled: {
      addListener: vi.fn(),
    },
  },
  tabs: {
    query: vi.fn(async () => []),
    sendMessage: vi.fn(async () => undefined),
  },
};

// Expose storage for test manipulation
export const __testStorage = storage;
export const __resetStorage = () => {
  for (const key of Object.keys(storage)) delete storage[key];
};

export default browser;
