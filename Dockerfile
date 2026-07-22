# syntax=docker/dockerfile:1.4
#
# Telegram Drive Server - Optimized Production Image
# Target: <400MB (from ~800MB)
#
# Build: docker build -t telegram-drive-server:4.0 --target runtime .
# Dev:   docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d --build
#        .\scripts\dev-up.ps1
#
# Techniques:
#   1. Multi-stage build (chef → deps → builder → runtime)
#   2. UPX binary compression (~40-60% size reduction)
#   3. debian:bookworm-slim minimal runtime
#   4. Cache mounts for Cargo registry/target
#   5. Strip debug symbols
#   6. Non-root user for security

ARG RUST_VERSION=1.85-bookworm
ARG DEBIAN_VERSION=bookworm-slim

# ═══════════════════════════════════════════════════════════════════════════
# Stage: base - Rust toolchain + build deps
# ═══════════════════════════════════════════════════════════════════════════
FROM rust:${RUST_VERSION} AS base
WORKDIR /build
# Headless server only — no GTK/WebKit dependencies (saves ~300MB in build stage)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev curl \
    && rm -rf /var/lib/apt/lists/*

FROM base AS chef-bin
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked,id=td-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked,id=td-cargo-git \
    cargo install cargo-chef --version 0.1.68 --locked

# planner 仅生成 recipe.json（快）；deps 不 COPY 业务 src，改 .rs 时不重编依赖
FROM chef-bin AS planner
WORKDIR /build/app/src-tauri
COPY app/src-tauri/Cargo.toml app/src-tauri/Cargo.lock ./
COPY app/src-tauri/build.rs app/src-tauri/tauri.conf.json ./
COPY app/src-tauri/src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked,id=td-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked,id=td-cargo-git \
    cargo chef prepare --recipe-path recipe.json

# ── 依赖层：仅 Cargo.lock / recipe 变更时重跑（不 COPY 业务 src）──
FROM chef-bin AS deps
WORKDIR /build/app/src-tauri
COPY app/src-tauri/Cargo.toml app/src-tauri/Cargo.lock ./
COPY app/src-tauri/build.rs app/src-tauri/tauri.conf.json ./
COPY --from=planner /build/app/src-tauri/recipe.json recipe.json
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_INCREMENTAL=1
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked,id=td-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked,id=td-cargo-git \
    --mount=type=cache,target=/build/app/src-tauri/target,sharing=locked,id=td-cargo-target \
    cargo chef cook --release --recipe-path recipe.json \
    -p app --bin telegram-drive-server --features headless-server

# ═══════════════════════════════════════════════════════════════════════════
# Stage: builder - Compile server binary with UPX compression
# ═══════════════════════════════════════════════════════════════════════════
FROM deps AS builder
WORKDIR /build/app/src-tauri

# Install UPX for binary compression (saves ~40-60% binary size)
RUN apt-get update && apt-get install -y --no-install-recommends upx \
    && rm -rf /var/lib/apt/lists/*

COPY app/src-tauri/src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked,id=td-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked,id=td-cargo-git \
    --mount=type=cache,target=/build/app/src-tauri/target,sharing=locked,id=td-cargo-target \
    cargo build --release -p app --bin telegram-drive-server --features headless-server \
    && mkdir -p /export \
    && cp target/release/telegram-drive-server /export/telegram-drive-server \
    && strip /export/telegram-drive-server \
    && upx --best --lzma /export/telegram-drive-server

# ── 开发层：依赖预编译 + cargo watch，源码通过 Volume 挂载，改 .rs 自动重编 ──
FROM deps AS dev
WORKDIR /build/app/src-tauri
RUN apt-get update && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked,id=td-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked,id=td-cargo-git \
    cargo install cargo-watch --locked
COPY app/src-tauri/src ./src
RUN mkdir -p /app/deploy/web /app/docs
ENV DATA_DIR=/data STATIC_DIR=/app/deploy/web DOCS_DIR=/app/docs PORT=1334 BIND_HOST=0.0.0.0 \
    CARGO_INCREMENTAL=1 RUST_LOG=info
EXPOSE 1334
HEALTHCHECK --interval=15s --timeout=5s --start-period=300s --retries=5 \
    CMD curl -fsS "http://127.0.0.1:${PORT:-1334}/health/live" || exit 1
# --poll：Windows 下 bind mount 文件变更检测；debug 编比 release 快，适合开发
CMD ["cargo", "watch", "--poll", "-d", "5", "-w", "src", "-w", "build.rs", \
     "-s", "cargo run --bin telegram-drive-server --features headless-server"]

# ═══════════════════════════════════════════════════════════════════════════
# Stage: runtime - Minimal production image
# ═══════════════════════════════════════════════════════════════════════════
FROM debian:${DEBIAN_VERSION} AS runtime

# Install only essential runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/* /var/log/* /tmp/* /var/tmp/*

# Create non-root user for security
RUN groupadd -r telegram && useradd -r -g telegram telegram

WORKDIR /app

# Copy compressed binary
COPY --from=builder --chown=telegram:telegram /export/telegram-drive-server /app/telegram-drive-server

# Copy static web assets
COPY --chown=telegram:telegram deploy/web /app/deploy/web
COPY --chown=telegram:telegram docs /app/docs

# Create data directory with proper permissions
RUN mkdir -p /data && chown telegram:telegram /data

ENV DATA_DIR=/data \
    STATIC_DIR=/app/deploy/web \
    DOCS_DIR=/app/docs \
    PORT=1334

EXPOSE 1334
VOLUME ["/data"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=40s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${PORT:-1334}/health/live" || exit 1

# Run as non-root user
USER telegram

CMD ["/app/telegram-drive-server"]
