#!/usr/bin/env bash
# Create 15 fake environments in ~/.dev-runner/ for scroll testing.
# Each environment has a meta file and several status files with varied states.
#
# Usage:   bash create-test-envs.sh
# Cleanup: bash create-test-envs.sh --clean

set -euo pipefail

DIR="$HOME/.dev-runner"
mkdir -p "$DIR"

# Hex IDs (must be all hex chars for is_meta_file())
IDS=(
  "a0a0a0a0a0a00001"
  "a0a0a0a0a0a00002"
  "a0a0a0a0a0a00003"
  "a0a0a0a0a0a00004"
  "a0a0a0a0a0a00005"
  "a0a0a0a0a0a00006"
  "a0a0a0a0a0a00007"
  "a0a0a0a0a0a00008"
  "a0a0a0a0a0a00009"
  "a0a0a0a0a0a0000a"
  "a0a0a0a0a0a0000b"
  "a0a0a0a0a0a0000c"
  "a0a0a0a0a0a0000d"
  "a0a0a0a0a0a0000e"
  "a0a0a0a0a0a0000f"
)

if [[ "${1:-}" == "--clean" ]]; then
  echo "Cleaning up test environments..."
  for id in "${IDS[@]}"; do
    rm -f "$DIR/$id" "$DIR/$id".*.status
  done
  echo "Done."
  exit 0
fi

NOW=$(date +%s)

# Helper: write meta file
# Usage: meta <id> <dir> <pid> <started> [PORT_KEYS...]
meta() {
  local id="$1" dir="$2" pid="$3" started="$4"
  shift 4
  {
    echo "DIR=$dir"
    echo "PID=$pid"
    echo "STARTED=$started"
    for pk in "$@"; do
      echo "$pk"
    done
  } > "$DIR/$id"
}

# Helper: write status file
# Usage: status <id> <unit_name> <state> [detail]
status() {
  local id="$1" unit="$2" state="$3" detail="${4:-}"
  if [[ -n "$detail" ]]; then
    echo "$state: $detail" > "$DIR/$id.$unit.status"
  else
    echo "$state" > "$DIR/$id.$unit.status"
  fi
}

# Environment 1: Web app with many services
meta "${IDS[0]}" "/home/user/projects/webapp" 99999 "$((NOW - 3600))" "SERVER_PORT=3000" "VITE_PORT=5173"
status "${IDS[0]}" "server" "ready"
status "${IDS[0]}" "vite" "running"
status "${IDS[0]}" "worker" "running"
status "${IDS[0]}" "db" "ready"
status "${IDS[0]}" "redis" "ready"

# Environment 2: API service
meta "${IDS[1]}" "/home/user/projects/api-service" 99998 "$((NOW - 7200))" "API_PORT=8080"
status "${IDS[1]}" "api" "ready"
status "${IDS[1]}" "migrations" "stopped"
status "${IDS[1]}" "scheduler" "running"

# Environment 3: Frontend
meta "${IDS[2]}" "/home/user/projects/frontend" 99997 "$((NOW - 120))" "DEV_PORT=3001"
status "${IDS[2]}" "dev" "building" "Compiling 42 modules"
status "${IDS[2]}" "lint" "running"
status "${IDS[2]}" "typecheck" "building"

# Environment 4: Microservice A
meta "${IDS[3]}" "/home/user/projects/auth-svc" 99996 "$((NOW - 300))" "AUTH_PORT=4000"
status "${IDS[3]}" "auth" "ready"
status "${IDS[3]}" "cache" "running"

# Environment 5: Microservice B
meta "${IDS[4]}" "/home/user/projects/payment-svc" 99995 "$((NOW - 600))" "PAYMENT_PORT=4001"
status "${IDS[4]}" "payment" "failed" "Connection refused to stripe"
status "${IDS[4]}" "webhook" "stopped"
status "${IDS[4]}" "queue" "running"

# Environment 6: Data pipeline
meta "${IDS[5]}" "/home/user/projects/data-pipeline" 99994 "$((NOW - 86400))"
status "${IDS[5]}" "ingest" "running"
status "${IDS[5]}" "transform" "building" "Step 3/7"
status "${IDS[5]}" "export" "stopped"
status "${IDS[5]}" "monitor" "ready"

# Environment 7: Mobile backend
meta "${IDS[6]}" "/home/user/projects/mobile-backend" 99993 "$((NOW - 1800))" "GRAPHQL_PORT=4444"
status "${IDS[6]}" "graphql" "ready"
status "${IDS[6]}" "push" "starting"
status "${IDS[6]}" "media" "running"
status "${IDS[6]}" "cdn" "ready"
status "${IDS[6]}" "analytics" "running"

# Environment 8: ML service
meta "${IDS[7]}" "/home/user/projects/ml-service" 99992 "$((NOW - 900))" "INFERENCE_PORT=5000"
status "${IDS[7]}" "inference" "ready"
status "${IDS[7]}" "training" "building" "Epoch 12/50"
status "${IDS[7]}" "preprocessing" "running"

# Environment 9: Admin dashboard
meta "${IDS[8]}" "/home/user/projects/admin-dash" 99991 "$((NOW - 45))" "ADMIN_PORT=3002"
status "${IDS[8]}" "admin" "starting"
status "${IDS[8]}" "api" "building"

# Environment 10: Docs site
meta "${IDS[9]}" "/home/user/projects/docs-site" 99990 "$((NOW - 7200))" "DOCS_PORT=8000"
status "${IDS[9]}" "docs" "ready"
status "${IDS[9]}" "search" "running"

# Environment 11: Notification service
meta "${IDS[10]}" "/home/user/projects/notif-svc" 99989 "$((NOW - 200))" "NOTIF_PORT=6000"
status "${IDS[10]}" "email" "running"
status "${IDS[10]}" "sms" "failed" "Twilio auth error"
status "${IDS[10]}" "push" "ready"
status "${IDS[10]}" "template" "running"

# Environment 12: Search service
meta "${IDS[11]}" "/home/user/projects/search-svc" 99988 "$((NOW - 3000))" "SEARCH_PORT=9200"
status "${IDS[11]}" "indexer" "running"
status "${IDS[11]}" "search" "ready"
status "${IDS[11]}" "reindex" "stopped"

# Environment 13: Chat service
meta "${IDS[12]}" "/home/user/projects/chat-svc" 99987 "$((NOW - 500))" "CHAT_PORT=7000" "WS_PORT=7001"
status "${IDS[12]}" "chat" "ready"
status "${IDS[12]}" "ws" "running"
status "${IDS[12]}" "presence" "running"
status "${IDS[12]}" "history" "ready"

# Environment 14: Monitoring stack
meta "${IDS[13]}" "/home/user/projects/monitoring" 99986 "$((NOW - 172800))" "GRAFANA_PORT=3333" "PROMETHEUS_PORT=9090"
status "${IDS[13]}" "grafana" "ready"
status "${IDS[13]}" "prometheus" "ready"
status "${IDS[13]}" "alertmanager" "running"
status "${IDS[13]}" "exporter" "running"
status "${IDS[13]}" "loki" "building" "Downloading WAL segments"

# Environment 15: CI runner
meta "${IDS[14]}" "/home/user/projects/ci-runner" 99985 "$((NOW - 60))"
status "${IDS[14]}" "runner" "starting"
status "${IDS[14]}" "cache" "building"
status "${IDS[14]}" "artifacts" "stopped"

echo "Created 15 test environments in $DIR"
echo "Total files: $(ls "$DIR" | wc -l)"
echo ""
echo "To clean up:  bash $(realpath "$0") --clean"
