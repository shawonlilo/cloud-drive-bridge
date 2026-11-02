#!/usr/bin/env bash
# Cloud Drive Bridge — end-to-end proof for the cdb CLI.
#
# Runs the REAL CLI against a REAL local S3 server and asserts every
# step: test -> put -> ls -> get (byte-compare) -> rename -> share
# link (actually fetched, byte-compare) -> size -> rm.
#
# Server: uses $CDB_E2E_ENDPOINT if a server is already running;
# otherwise starts MinIO if a `minio` binary is on PATH, else a moto
# server (`pip install "moto[server]"`). Credentials are generated at
# runtime — no secrets live in this script.
#
# Usage: scripts/e2e-minio.sh [path-to-cdb-binary]
set -u
CDB="${1:-cdb}"
PORT="${CDB_E2E_PORT:-5599}"
ENDPOINT="http://127.0.0.1:${PORT}"
BUCKET="cdb-e2e"
REGION="us-east-1"
WORK="$(mktemp -d)"
SERVER_PID=""
FAILED=0

export CDB_ACCESS_KEY_ID="e2e-$(head -c 6 /dev/urandom | od -An -tx1 | tr -d ' \n')"
export CDB_SECRET_ACCESS_KEY="$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')"

cleanup() {
  [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null
  rm -rf "$WORK"
}
trap cleanup EXIT

step() { echo; echo "== $*"; }
pass() { echo "PASS: $*"; }
fail() { echo "FAIL: $*"; FAILED=1; }

# ------------------------------------------------------------- server boot
if [ -n "${CDB_E2E_ENDPOINT:-}" ]; then
  ENDPOINT="$CDB_E2E_ENDPOINT"
  echo "Using external S3 endpoint: $ENDPOINT"
elif command -v minio >/dev/null 2>&1; then
  mkdir -p "$WORK/data"
  MINIO_ROOT_USER="$CDB_ACCESS_KEY_ID" MINIO_ROOT_PASSWORD="$CDB_SECRET_ACCESS_KEY" \
    minio server "$WORK/data" --address "127.0.0.1:${PORT}" >"$WORK/server.log" 2>&1 &
  SERVER_PID=$!
  echo "Started MinIO (pid $SERVER_PID) on $ENDPOINT"
elif command -v moto_server >/dev/null 2>&1; then
  moto_server -p "$PORT" -H 127.0.0.1 >"$WORK/server.log" 2>&1 &
  SERVER_PID=$!
  echo "Started moto server (pid $SERVER_PID) on $ENDPOINT"
else
  echo "No S3 server available: install minio or run: pip install \"moto[server]\""
  exit 2
fi
for i in $(seq 1 30); do
  (echo > "/dev/tcp/127.0.0.1/${PORT}") 2>/dev/null && break
  sleep 1
done

# Create the bucket with a small stdlib SigV4 signer (test tooling only).
python3 - "$ENDPOINT" "$BUCKET" <<'PYEOF'
import sys, hashlib, hmac, datetime, urllib.request
endpoint, bucket = sys.argv[1], sys.argv[2]
import os
ak, sk = os.environ["CDB_ACCESS_KEY_ID"], os.environ["CDB_SECRET_ACCESS_KEY"]
now = datetime.datetime.now(datetime.timezone.utc)
amz = now.strftime("%Y%m%dT%H%M%SZ"); day = now.strftime("%Y%m%d")
host = endpoint.split("://", 1)[1]
payload = hashlib.sha256(b"").hexdigest()
headers = {"host": host, "x-amz-content-sha256": payload, "x-amz-date": amz}
signed = ";".join(headers)
canon = "PUT\n/%s/\n\n%s\n%s\n%s" % (bucket, "".join("%s:%s\n" % kv for kv in headers.items()), signed, payload)
scope = "%s/us-east-1/s3/aws4_request" % day
sts = "AWS4-HMAC-SHA256\n%s\n%s\n%s" % (amz, scope, hashlib.sha256(canon.encode()).hexdigest())
def H(k, m): return hmac.new(k, m.encode(), hashlib.sha256).digest()
key = H(H(H(H(("AWS4" + sk).encode(), day), "us-east-1"), "s3"), "aws4_request")
sig = hmac.new(key, sts.encode(), hashlib.sha256).hexdigest()
req = urllib.request.Request(endpoint + "/" + bucket + "/", method="PUT")
req.add_header("Authorization", "AWS4-HMAC-SHA256 Credential=%s/%s, SignedHeaders=%s, Signature=%s" % (ak, scope, signed, sig))
req.add_header("x-amz-date", amz); req.add_header("x-amz-content-sha256", payload)
try:
    urllib.request.urlopen(req, timeout=10)
    print("bucket created")
except Exception as e:
    print("bucket create failed:", e); sys.exit(1)
PYEOF
[ $? -eq 0 ] || { echo "FAIL: bucket setup"; exit 1; }

CDB_ARGS="--provider minio --endpoint $ENDPOINT --bucket $BUCKET --region $REGION"
printf 'cloud-drive-bridge e2e payload — the quick brown fox\n' > "$WORK/payload.txt"
printf 'cloud-drive-bridge e2e payload — the quick brown fox\n' > "$WORK/expected.txt"

step "1. test connection (5-point check incl. write/delete probe)"
OUT="$("$CDB" $CDB_ARGS test --probe 2>&1)" && echo "$OUT" || fail "test"
echo "$OUT" | grep -q '"write":true' && echo "$OUT" | grep -q '"delete":true' \
  && pass "5-point: reachable/auth/list/write/delete all true" || fail "5-point probe: $OUT"

step "2. put (upload + checksum verification)"
OUT="$("$CDB" $CDB_ARGS put "$WORK/payload.txt" docs/payload.txt 2>&1)" && echo "$OUT" || fail "put"
echo "$OUT" | grep -q "checksum: verified" && pass "put checksum verified" || fail "put checksum: $OUT"

step "3. ls"
OUT="$("$CDB" $CDB_ARGS ls docs/ 2>&1)" && echo "$OUT" || fail "ls"
echo "$OUT" | grep -q "payload.txt" && pass "ls shows payload.txt" || fail "ls content"

step "4. get + byte-compare"
"$CDB" $CDB_ARGS get docs/payload.txt "$WORK/got.txt" >/dev/null 2>&1 && cmp -s "$WORK/got.txt" "$WORK/expected.txt" \
  && pass "get byte-identical" || fail "get/compare"

step "5. rename"
"$CDB" $CDB_ARGS rename docs/payload.txt docs/renamed.txt >/dev/null 2>&1 \
  && "$CDB" $CDB_ARGS ls docs/ 2>&1 | grep -q "renamed.txt" && pass "rename" || fail "rename"

step "6. share link really fetches"
URL="$("$CDB" $CDB_ARGS share docs/renamed.txt 1h 2>&1 | tail -n 1 | cut -d' ' -f1)"
echo "$URL" | grep -q "X-Amz-Signature=" || fail "share url shape: $URL"
python3 - "$URL" "$WORK/shared.txt" <<'PYEOF'
import sys, urllib.request
try:
    data = urllib.request.urlopen(sys.argv[1], timeout=15).read()
    open(sys.argv[2], "wb").write(data)
except Exception as e:
    print("fetch failed:", e); sys.exit(1)
PYEOF
[ $? -eq 0 ] && cmp -s "$WORK/shared.txt" "$WORK/expected.txt" && pass "share link fetched byte-identical" || fail "share fetch"

step "7. size"
OUT="$("$CDB" $CDB_ARGS size 2>&1)" && echo "$OUT" || fail "size"
echo "$OUT" | grep -q "1 objects" && pass "size counts 1 object" || fail "size content: $OUT"

step "8. rm"
"$CDB" $CDB_ARGS rm docs/renamed.txt >/dev/null 2>&1 \
  && OUT="$("$CDB" $CDB_ARGS ls docs/ 2>&1)" && ! echo "$OUT" | grep -q "renamed.txt" \
  && echo "$OUT" | grep -q "0 item(s)" && pass "rm (folder empty after)" || fail "rm"

echo
if [ "$FAILED" -eq 0 ]; then echo "E2E RESULT: ALL STEPS PASS ✅"; else echo "E2E RESULT: FAILURES ❌"; exit 1; fi
