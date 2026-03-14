# 015 Zod Runtime Stabilization Checklist

## Stabilization Smoke Baseline (FR-1)

- [ ] Place live limit trade from extension modal.
- [ ] Confirm exchange accepts entry order.
- [ ] Verify trade appears in extension active positions list.
- [ ] Cancel/close trade and confirm no orphaned SL/TP pending orders remain.

## Release Gate (FR-10)

- [ ] `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- [ ] `cd testudo-extension && bun run typecheck && bun run test && bun run build`
- [ ] `cd testudo-web && bun run lint && bun run build`
