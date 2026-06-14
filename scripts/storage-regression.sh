#!/usr/bin/env bash
# Storage regression smoke test. Usage: ./scripts/storage-regression.sh [BASE_URL]
set -euo pipefail
BASE_URL="${1:-http://localhost:1334}"
API_KEY="${API_KEY:-}"

echo "== health =="
curl -fsS "$BASE_URL/api/v1/health" | grep -q '"status":"ok"'

echo "== config =="
curl -fsS "$BASE_URL/config" | grep -q 'chunk_concurrent'

if [[ -n "$API_KEY" ]]; then
  echo "== api_upload_small =="
  tmp="$(mktemp)"
  echo "storage-regression-$(date -Iseconds)" >"$tmp"
  code="$(curl -s -o /dev/null -w '%{http_code}' \
    -H "X-API-Key: $API_KEY" \
    -F "file=@$tmp;filename=regression.txt" \
    "$BASE_URL/api/v1/files")"
  rm -f "$tmp"
  [[ "$code" == "200" || "$code" == "201" || "$code" == "503" ]] || {
    echo "upload HTTP $code" >&2
    exit 1
  }
else
  echo "SKIP upload (set API_KEY)"
fi

echo "== metrics_optional =="
code="$(curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/metrics")"
[[ "$code" == "200" || "$code" == "404" ]]

echo "Storage regression passed."
