# syntax=docker/dockerfile:1.4
#
# Telegram Drive Server - Python (FastAPI + Telethon) headless image.
# No compilation of the app itself: pure wheel install, small and fast to build.
#
# Build: docker build -t telegram-drive-api:local .
# Dev:   docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d --build
# Prod:  docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d --build

ARG PYTHON_VERSION=3.11-slim-bookworm

# ═══════════════════════════════════════════════════════════════════════════
# Stage: deps - install Python wheels into an isolated prefix
# ═══════════════════════════════════════════════════════════════════════════
FROM python:${PYTHON_VERSION} AS deps
WORKDIR /build
COPY backend/requirements.txt ./requirements.txt
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install --no-compile --prefix=/install -r requirements.txt

# ═══════════════════════════════════════════════════════════════════════════
# Stage: dev - uvicorn --reload over bind-mounted source
# ═══════════════════════════════════════════════════════════════════════════
FROM python:${PYTHON_VERSION} AS dev
COPY --from=deps /install /usr/local
WORKDIR /app
RUN mkdir -p /app/deploy/web /app/docs /data
ENV DATA_DIR=/data STATIC_DIR=/app/deploy/web DOCS_DIR=/app/docs \
    PORT=1334 BIND_HOST=0.0.0.0 PYTHONUNBUFFERED=1
EXPOSE 1334
HEALTHCHECK --interval=15s --timeout=5s --start-period=30s --retries=5 \
    CMD python -c "import urllib.request,sys;sys.exit(0 if urllib.request.urlopen('http://127.0.0.1:1334/health/live',timeout=3).status==200 else 1)" || exit 1
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "1334", "--reload", "--app-dir", "/app/backend"]

# ═══════════════════════════════════════════════════════════════════════════
# Stage: runtime - minimal production image
# ═══════════════════════════════════════════════════════════════════════════
FROM python:${PYTHON_VERSION} AS runtime
COPY --from=deps /install /usr/local

# Non-root user for security.
RUN groupadd -r telegram && useradd -r -g telegram telegram

WORKDIR /app
COPY --chown=telegram:telegram backend/app /app/backend/app
COPY --chown=telegram:telegram deploy/web /app/deploy/web
COPY --chown=telegram:telegram docs /app/docs
RUN mkdir -p /data && chown telegram:telegram /data

ENV DATA_DIR=/data \
    STATIC_DIR=/app/deploy/web \
    DOCS_DIR=/app/docs \
    PORT=1334 \
    PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1

EXPOSE 1334
VOLUME ["/data"]

# python:slim ships no curl; probe the liveness endpoint with stdlib urllib.
HEALTHCHECK --interval=30s --timeout=5s --start-period=40s --retries=3 \
    CMD python -c "import urllib.request,sys;sys.exit(0 if urllib.request.urlopen('http://127.0.0.1:1334/health/live',timeout=3).status==200 else 1)" || exit 1

USER telegram
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "1334", "--app-dir", "/app/backend"]
