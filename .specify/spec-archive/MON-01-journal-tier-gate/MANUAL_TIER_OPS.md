# Manual Tier Operations (pre-MON-02)

Until MON-02 wires a payment rail, tier promotion is hand-traded via SQL
against the prod `users` table. Keep this file alongside the spec — it
is the operational runbook for the first paying cohort.

---

## One-shot: grandfather all existing users (run at MON-01 deploy)

Every existing user has been using the journal under the assumption that
all their data stays visible. Flipping the gate cold-turkey will look
like data loss. Grant a 30-day pro trial to every existing account so
the in-app upgrade prompt has a window to land:

```sql
UPDATE users
SET tier = 'pro',
    tier_expires_at = NOW() + INTERVAL '30 days'
WHERE tier = 'free';
```

Run this **once**, immediately after the migration applies in prod, and
**before** the journal frontend ships the gating UI. Verify count:

```sql
SELECT COUNT(*) FROM users WHERE tier = 'pro' AND tier_expires_at IS NOT NULL;
```

---

## Promote to PRO (lifetime — for paid reservers)

```sql
UPDATE users
SET tier = 'pro',
    tier_expires_at = NULL
WHERE wallet_address = $1;
```

Use after a reserver has paid (off-band — bank transfer, USDC send, NFT
mint, whatever the payment story ends up being). `NULL` expiry = lifetime.

---

## Promote to PRO (time-bounded — for trials, beta testers, comp)

```sql
UPDATE users
SET tier = 'pro',
    tier_expires_at = NOW() + INTERVAL '30 days'
WHERE wallet_address = $1;
```

Adjust interval as needed: `'7 days'`, `'1 year'`, etc.

---

## Demote (refund, churn, expiry override)

```sql
UPDATE users
SET tier = 'free',
    tier_expires_at = NULL
WHERE wallet_address = $1;
```

Demoting takes effect on the **next** request. There is no JWT
invalidation needed because tier is read per-request, not cached
in the token (see MON-01 spec, Risk 6).

---

## Audit: who is currently effectively PRO?

```sql
SELECT id,
       wallet_address,
       tier,
       tier_expires_at,
       CASE
         WHEN tier_expires_at IS NULL THEN 'lifetime'
         WHEN tier_expires_at > NOW() THEN 'active'
         ELSE 'expired'
       END AS status
FROM users
WHERE tier = 'pro'
ORDER BY tier_expires_at NULLS FIRST;
```

---

## Audit: pro users expiring in the next 7 days

```sql
SELECT wallet_address, tier_expires_at
FROM users
WHERE tier = 'pro'
  AND tier_expires_at IS NOT NULL
  AND tier_expires_at BETWEEN NOW() AND NOW() + INTERVAL '7 days'
ORDER BY tier_expires_at ASC;
```

Use this to send "your trial expires soon" nudges (manual email until
MON-02 ships a notification job).

---

## Reserver capture (until MON-02)

The pricing page's `[ RESERVE WITH WALLET ]` button links to
`https://desk.testudo.vip?reserve=pro`. Any wallet that completes SIWE
with that query param is a reserver candidate.

Until MON-02 adds explicit reservation tracking, identify reservers
from the auth/session logs:

```bash
# rough — adjust to actual log format
grep 'reserve=pro' /var/log/testudo/router.log | grep 'verify-siwe success' | sort -u
```

Then promote whichever wallets you've confirmed payment from using the
lifetime-promote query above.

---

## Safety notes

- **Always filter by `wallet_address`, not `email` or `id`.** Wallet is
  the canonical identity (post-AUTH-02). Promoting the wrong account
  silently is unrecoverable from a UX perspective.
- **Check `wallet_address` case.** Schema is `VARCHAR(48)`, not
  case-folded. Use `LOWER(wallet_address) = LOWER($1)` if there's any
  doubt.
- **Wrap in a transaction for batch operations.** Single-row updates
  are fine raw, but if you're running a list of promotions, use
  `BEGIN; ... COMMIT;` so a typo in one statement doesn't leave the
  table half-updated.
- **Keep a paper trail.** When you promote a paid user, log it
  somewhere (a private Notion page, a `payments.txt` file, anything
  durable) — wallet, amount paid, payment medium, date. MON-02 will
  consume this when it backfills payment records into a real table.
