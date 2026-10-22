# SPDX-License-Identifier: AGPL-3.0-only
# This file is part of Daywerk. See LICENSE for details.

# ---- Stage 1: web UI ---------------------------------------------------
FROM docker.io/node:22-alpine AS web-builder
WORKDIR /src
RUN corepack enable
COPY pnpm-workspace.yaml package.json pnpm-lock.yaml ./
COPY apps/web-ui/package.json apps/web-ui/
RUN pnpm install --frozen-lockfile
COPY apps/web-ui/ apps/web-ui/
RUN pnpm --filter web-ui build

# ---- Stage 2: server ---------------------------------------------------
FROM docker.io/rust:1-bookworm AS rust-builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
# Cargo resolves every workspace member, and the Tauri shell is one of them
# (its sources are not compiled here, only its manifest is needed).
COPY apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.toml
COPY apps/desktop/src-tauri/build.rs apps/desktop/src-tauri/build.rs
COPY apps/desktop/src-tauri/src/ apps/desktop/src-tauri/src/
RUN cargo build --release -p dw-server

# ---- Stage 3: runtime --------------------------------------------------
FROM docker.io/debian:bookworm-slim
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-builder /src/target/release/dw-server /app/dw-server
COPY --from=web-builder /src/apps/web-ui/dist /app/web

ENV DB_PATH=/app/data/daywerk.db
ENV WEB_ROOT=/app/web
ENV PORT=9095
EXPOSE 9095
VOLUME /app/data

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s \
  CMD curl -fsS http://127.0.0.1:9095/api/health || exit 1

CMD ["/app/dw-server"]
