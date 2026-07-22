# syntax=docker/dockerfile:1.4
#
# 基础镜像：固定版本号（禁止 latest）
#   rust:1.85-bookworm  /  debian:bookworm-slim
# 本地开发（推荐）：Volume 挂载 + cargo watch，改代码不 docker build
#   docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d --build   # 仅首次/依赖变更
#   .\scripts\dev-up.ps1
# 生产/交付：全量镜像
#   docker compose up -d --build
# 增量 docker build（仅 CI 或不用 dev compose 时）：
#   .\scripts\dev-build-rust.ps1
#
# 层分离：Cargo.lock → deps(cook) → COPY src → build
# 注意：target 在 cache mount 中，必须把二进制 cp 到 /export 才能被下一阶段 COPY

ARG RUST_VERSION=1.85-bookworm
ARG DEBIAN_VERSION=bookworm-slim

FROM rust:${RUST_VERSION} AS base
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    libwebkit2gtk-4.1-dev libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev \
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

# ── 业务层：只 COPY src + 链接（改 .rs 时主要耗时在此，复用 target 缓存）──
FROM deps AS builder
WORKDIR /build/app/src-tauri
COPY app/src-tauri/src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked,id=td-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked,id=td-cargo-git \
    --mount=type=cache,target=/build/app/src-tauri/target,sharing=locked,id=td-cargo-target \
    cargo build --release -p app --bin telegram-drive-server --features headless-server \
    && mkdir -p /export \
    && cp target/release/telegram-drive-server /export/telegram-drive-server \
    && strip /export/telegram-drive-server

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
    CMD curl -fsS http://127.0.0.1:1334/api/v1/health || exit 1
# --poll：Windows 下 bind mount 文件变更检测；debug 编比 release 快，适合开发
CMD ["cargo", "watch", "--poll", "-d", "5", "-w", "src", "-w", "build.rs", \
     "-s", "cargo run --bin telegram-drive-server --features headless-server"]

# ── 运行时：无 Rust 工具链 ──
FROM debian:${DEBIAN_VERSION} AS runtime
# Headless API only — no GTK/WebKit (saves ~200MB vs desktop deps)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /export/telegram-drive-server /app/telegram-drive-server
COPY deploy/web /app/deploy/web
COPY docs /app/docs

ENV DATA_DIR=/data STATIC_DIR=/app/deploy/web DOCS_DIR=/app/docs PORT=1334 BIND_HOST=0.0.0.0
EXPOSE 1334
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=40s --retries=3 \
    CMD curl -fsS http://127.0.0.1:1334/api/v1/health || exit 1
CMD ["/app/telegram-drive-server"]
