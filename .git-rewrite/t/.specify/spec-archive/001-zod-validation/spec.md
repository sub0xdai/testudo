# Specification: Add Zod Runtime Validation

## Overview
Add runtime validation with Zod to the TypeScript/JavaScript parts of Testudo to prevent runtime errors from malformed API responses, invalid environment variables, and unsafe type assertions.

Current: 30+ unsafe `as` assertions across API response handling, JWT parsing, and TradingView scraping; manual `typeof` checks; no validation of backend contracts.
Target: All high-risk external data (API responses, JWT payloads, trade execution parameters) validated with Zod schemas before use. 

---
## Functional Requirements
| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add `zod` dependency to both testudo-web and testudo-extension `package.json` | High | Build |
| FR-2 | Create Zod schemas for all core types in `testudo-extension/src/schemas.ts` (Duplicate locally for now) | High | Validation |
| FR-3 | Replace all `as` type assertions in `background.ts` high-risk API responses with schema parsing | High | API Safety |
| FR-4 | Validate JWT payloads in `background.ts` lines 129, 583, 639 with Zod schemas | High | Security |
| FR-5 | Replace manual `typeof` checks in `scraper.ts` with structured Zod `.safeParse()` | High | Data Extraction |
| FR-6 | Add environment variable validation for `VITE_API_URL` in testudo-web | Medium | Configuration |
| FR-7 | Add form validation with Zod to login/register/exchange forms in testudo-web | Medium | UX |
| FR-8 | Validate stored settings in `background.ts` `getSettings()` function | Medium | Configuration |

---
## Technical Implementation

### 1. Dependency Addition (FR-1)
```bash
cd testudo-web && bun add zod
cd ../testudo-extension && bun add zod
```

### 2. Core Schema Definitions (FR-2)
Create `/testudo-extension/src/schemas.ts`:
```typescript
import { z } from "zod";

export const AuthTokensSchema = z.object({
  access_token: z.string(),
  refresh_token: z.string(),
  expires_in: z.number(),
});

export const TradePayloadSchema = z.object({
  symbol: z.string(),
  side: z.enum(["LONG", "SHORT"]),
  entry: z.number().positive(),
  stop: z.number().positive(),
  target: z.number().positive(),
  timeframe: z.string(),
  exchange_account_id: z.string().optional(),
  management: z.object({
    risk_percent: z.number().min(0.1).max(100),
    break_even_at: z.number().min(0).max(100),
    leverage: z.number().min(1).max(100),
    trailing_stop: z.object({ enabled: z.boolean(), distance_percent: z.number() }),
    partial_tp: z.object({ enabled: z.boolean(), close_percent: z.number() }),
  }),
});
```

### 3. Execution & API Response Validation (FR-3)
Use strict parsing (`.parse`) for critical paths. If it fails, throw and block execution.
```typescript
// testudo-extension/src/background.ts
try {
  const raw = await response.json();
  const payload = TradePayloadSchema.parse(raw);
  executeTrade(payload);
} catch (e) {
  console.error("Trade execution blocked: Malformed data", e);
  throw new Error("Critical: Malformed trade payload rejected.");
}
```

### 4. Scraper Validation Strategy (FR-5)
Use safe parsing (`.safeParse`) for the DOM scraper to prevent isolated bad ticks from crashing the background worker.
```typescript
// testudo-extension/src/scraper.ts
const result = ScrapedDataSchema.safeParse(domData);
if (!result.success) {
  console.warn("Dropped malformed price tick", result.error);
  return; 
}
const data = result.data;
```

### 5. Environment Variable Validation (FR-6)
Create `/testudo-web/src/config/env.ts`:
```typescript
import { z } from "zod";

const EnvSchema = z.object({
  VITE_API_URL: z.string().url(),
});

export const env = EnvSchema.parse(import.meta.env);
```

---
## Completion Signal
When ALL above criteria are satisfied and tests pass, output:
`<promise>DONE</promise>`
