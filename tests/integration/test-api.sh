#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${1:-http://localhost:1334}"
ACCESS_PWD="${2:-test}"
API_KEY="${3:-}"

pass=0
fail=0

ok() { echo "[PASS] $1"; pass=$((pass + 1)); }
bad() { echo "[FAIL] $1 — $2"; fail=$((fail + 1)); }

echo "Testing $BASE_URL"

if curl -fsS "$BASE_URL/api/v1/health" | grep -q '"status":"ok"'; then
  ok health
else
  bad health "unexpected response"
fi

health_json=$(curl -fsS "$BASE_URL/api/v1/health")
echo "$health_json" | grep -q '"telegram_connected"' && ok health_telegram_field || bad health_telegram_field "missing field"
echo "$health_json" | grep -q '"uptime_secs"' && ok health_uptime || bad health_uptime "missing field"
echo "$health_json" | grep -q '"ready"' && ok health_ready || bad health_ready "missing field"
echo "$health_json" | grep -q '"build"' && ok health_build || bad health_build "missing field"
echo "$health_json" | grep -q '"upload_queue"' && ok health_upload_queue || bad health_upload_queue "missing field"

if echo "$health_json" | grep -qE '"version":"[^"]+"'; then
  ok health_version
else
  bad health_version "missing version in health"
fi

if curl -fsS "$BASE_URL/config" | grep -q chunk_size_mb; then
  ok config
else
  bad config "missing fields"
fi

if curl -fsS -X POST "$BASE_URL/verify" --data-urlencode "pwd=$ACCESS_PWD" | grep -q ok; then
  ok verify
else
  bad verify "auth failed"
fi

if curl -fsS -X POST "$BASE_URL/verify" -F "pwd=$ACCESS_PWD" | grep -q ok; then
  ok verify_multipart
else
  bad verify_multipart "auth failed"
fi

chunk_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/upload_chunk" -F "pwd=$ACCESS_PWD")
if [ "$chunk_code" = "400" ] || [ "$chunk_code" = "401" ]; then
  ok upload_chunk_route
else
  bad upload_chunk_route "http $chunk_code"
fi

merge_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/merge_chunks" --data-urlencode "pwd=$ACCESS_PWD")
if [ "$merge_code" = "400" ] || [ "$merge_code" = "401" ]; then
  ok merge_chunks_route
else
  bad merge_chunks_route "http $merge_code"
fi

merge_mp=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/merge_chunks" -F "pwd=$ACCESS_PWD" -F "filename=x.bin" -F 'chunk_ids=[]')
if [ "$merge_mp" = "400" ] || [ "$merge_mp" = "503" ]; then
  ok merge_chunks_multipart
else
  bad merge_chunks_multipart "http $merge_mp"
fi

if curl -fsS "$BASE_URL/upload.html" | grep -q TdUpload; then
  ok upload_html
else
  bad upload_html "missing page"
fi

if curl -fsS "$BASE_URL/assets/upload-core.js" | grep -q merge_chunks; then
  ok upload_core_js
else
  bad upload_core_js "missing script"
fi

d_code=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/d?file_id=1&filename=test.bin")
if [ "$d_code" = "400" ] || [ "$d_code" = "503" ]; then
  ok legacy_download_route
else
  bad legacy_download_route "http $d_code"
fi

if [ -n "$API_KEY" ]; then
  if curl -fsS -H "X-API-Key: $API_KEY" "$BASE_URL/api/v1/auth/status" >/dev/null; then
    ok auth_status
  else
    bad auth_status "bad response"
  fi
  code=$(curl -s -o /dev/null -w "%{http_code}" -H "X-API-Key: $API_KEY" "$BASE_URL/api/v1/folders")
  if [ "$code" = "200" ] || [ "$code" = "503" ]; then
    ok folders
  else
    bad folders "http $code"
  fi
  qr_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/api/v1/auth/qr/start")
  if [ "$qr_code" = "200" ] || [ "$qr_code" = "503" ] || [ "$qr_code" = "400" ]; then
    ok qr_start
  else
    bad qr_start "http $qr_code"
  fi
  qr_poll=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/api/v1/auth/qr/poll")
  if [ "$qr_poll" = "200" ] || [ "$qr_poll" = "503" ] || [ "$qr_poll" = "400" ]; then
    ok qr_poll
  else
    bad qr_poll "http $qr_poll"
  fi
  shares_code=$(curl -s -o /dev/null -w "%{http_code}" -H "X-API-Key: $API_KEY" "$BASE_URL/api/v1/shares")
  if [ "$shares_code" = "200" ]; then
    ok shares_list
  else
    bad shares_list "http $shares_code"
  fi
  upload_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST -H "X-API-Key: $API_KEY" "$BASE_URL/api/v1/files")
  if [ "$upload_code" = "400" ] || [ "$upload_code" = "503" ]; then
    ok files_multipart_route
  else
    bad files_multipart_route "http $upload_code"
  fi
else
  echo "[SKIP] API key tests"
fi

echo "Result: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
