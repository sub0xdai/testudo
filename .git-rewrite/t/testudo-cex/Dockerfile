# testudo-cex sidecar — build context must be monorepo root
FROM oven/bun:1-alpine
WORKDIR /app
COPY testudo-cex/package.json testudo-cex/bun.lock ./
COPY safe-cex-sub0/ /app/vendor/safe-cex-sub0/
RUN sed -i 's|file:../safe-cex-sub0|file:./vendor/safe-cex-sub0|' package.json
RUN bun install --frozen-lockfile
COPY testudo-cex/src/ ./src/
COPY testudo-cex/tsconfig.json ./
EXPOSE 3100
CMD ["bun", "run", "src/server.ts"]
