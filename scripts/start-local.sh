#!/usr/bin/env bash

set -Eeuo pipefail

readonly ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly BACKEND_DIR="$ROOT_DIR/backend"
readonly FRONTEND_DIR="$ROOT_DIR/frontend"
readonly BACKEND_ENV_FILE="$BACKEND_DIR/.env"
readonly FRONTEND_ENV_FILE="$FRONTEND_DIR/.env.local"
readonly FIREBASE_CLI="$BACKEND_DIR/node_modules/.bin/firebase"

readonly POSTGRES_PORT=5432
readonly FIREBASE_AUTH_PORT=9099
readonly FIREBASE_STORAGE_PORT=9199
readonly FIREBASE_UI_PORT=4000
readonly BACKEND_PORT=3001
readonly FRONTEND_PORT=3000

child_pids=()

fail() {
  echo "Error: $*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "'$1' is required but was not found on PATH."
}

require_file() {
  [[ -f "$1" ]] || fail "Missing $1. Copy its .example file and configure it first."
}

listener_pids() {
  lsof -nP -t -iTCP:"$1" -sTCP:LISTEN 2>/dev/null || true
}

require_available_port() {
  local port="$1"
  local pids

  pids="$(listener_pids "$port")"
  [[ -z "$pids" ]] || fail "Port $port is already in use by PID(s): $pids. Stop the conflicting service or configure a different port."
}

configured_port() {
  local variable_name="$1"
  local default_port="$2"
  local port="${!variable_name:-$default_port}"

  [[ "$port" =~ ^[0-9]+$ ]] && ((port >= 1 && port <= 65535)) \
    || fail "$variable_name must be an integer between 1 and 65535."

  printf '%s' "$port"
}

release_project_service_port() {
  local service_name="$1"
  local service_label="$2"
  local port="$3"
  local service_id

  service_id="$(docker compose --project-directory "$ROOT_DIR" ps --quiet "$service_name")"
  if [[ -n "$service_id" ]]; then
    echo "Stopping this project's existing $service_label container..."
    docker compose --project-directory "$ROOT_DIR" stop "$service_name" >/dev/null
  fi

  require_available_port "$port"
}

release_postgres_port() {
  release_project_service_port postgres PostgreSQL "$POSTGRES_PORT"
}

release_clamav_port() {
  release_project_service_port clamav ClamAV "$1"
}

wait_for_port() {
  local name="$1"
  local port="$2"
  local pid="$3"
  local attempt

  for attempt in {1..60}; do
    [[ -n "$(listener_pids "$port")" ]] && return

    if ! kill -0 "$pid" 2>/dev/null; then
      wait "$pid" || true
      fail "$name exited before opening port $port."
    fi

    sleep 0.5
  done

  fail "$name did not open port $port within 30 seconds."
}

wait_for_clamav() {
  local attempt

  for attempt in {1..120}; do
    if docker compose --project-directory "$ROOT_DIR" exec --no-TTY clamav \
      sh -c "printf 'zPING\\0' | nc -w 1 127.0.0.1 3310 | tr -d '\\000' | grep -qx PONG" \
      >/dev/null 2>&1; then
      return
    fi

    if [[ -z "$(docker compose --project-directory "$ROOT_DIR" ps --status running --quiet clamav)" ]]; then
      fail "ClamAV exited before answering zPING. Inspect it with: docker compose logs clamav"
    fi
    sleep 0.5
  done

  fail "ClamAV did not answer zPING within 60 seconds. Inspect it with: docker compose logs clamav"
}

firebase_project_id() {
  local project_id

  project_id="${FIREBASE_PROJECT_ID:-}"
  if [[ -z "$project_id" ]]; then
    project_id="$(awk -F= '$1 == "FIREBASE_PROJECT_ID" { print substr($0, index($0, "=") + 1); exit }' "$BACKEND_ENV_FILE")"
  fi

  project_id="${project_id%$'\r'}"
  project_id="${project_id#\"}"
  project_id="${project_id%\"}"
  [[ -n "$project_id" && "$project_id" != "your-firebase-project-id" ]] \
    || fail "Set FIREBASE_PROJECT_ID in backend/.env before starting the Firebase emulator."

  printf '%s' "$project_id"
}

reset_database() {
  echo "Starting PostgreSQL..."
  docker compose --project-directory "$ROOT_DIR" up --detach postgres

  local attempt
  for attempt in {1..60}; do
    if docker compose --project-directory "$ROOT_DIR" exec --no-TTY postgres \
      pg_isready --username app --dbname app >/dev/null 2>&1; then
      break
    fi
    sleep 0.5
  done

  docker compose --project-directory "$ROOT_DIR" exec --no-TTY postgres \
    pg_isready --username app --dbname app >/dev/null 2>&1 \
    || fail "PostgreSQL did not become ready within 30 seconds."

  echo "Resetting the local PostgreSQL public schema..."
  docker compose --project-directory "$ROOT_DIR" exec --no-TTY postgres \
    psql --username app --dbname app --set ON_ERROR_STOP=1 <<'SQL'
DROP SCHEMA IF EXISTS public CASCADE;
CREATE SCHEMA public AUTHORIZATION app;
SQL
}

start_clamav() {
  echo "Starting ClamAV..."
  docker compose --project-directory "$ROOT_DIR" up --detach clamav
  wait_for_clamav
}

cleanup() {
  local exit_code=$?
  local pid

  trap - EXIT INT TERM
  for pid in "${child_pids[@]:-}"; do
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
    fi
  done
  for pid in "${child_pids[@]:-}"; do
    wait "$pid" 2>/dev/null || true
  done
  exit "$exit_code"
}

main() {
  require_command bun
  require_command cargo
  require_command docker
  require_command lsof
  docker info >/dev/null 2>&1 || fail "Docker Desktop is not running."
  require_file "$BACKEND_ENV_FILE"
  require_file "$FRONTEND_ENV_FILE"
  [[ -x "$FIREBASE_CLI" ]] || fail "Firebase tooling is not installed. Run: cd backend && bun install"

  local firebase_project
  firebase_project="$(firebase_project_id)"
  local clamav_port
  clamav_port="$(configured_port CLAMAV_HOST_PORT 3310)"
  export CLAMAV_HOST_PORT="$clamav_port"

  trap cleanup EXIT INT TERM

  require_available_port "$FRONTEND_PORT"
  require_available_port "$BACKEND_PORT"
  require_available_port "$FIREBASE_UI_PORT"
  require_available_port "$FIREBASE_AUTH_PORT"
  require_available_port "$FIREBASE_STORAGE_PORT"
  release_postgres_port
  release_clamav_port "$clamav_port"

  reset_database
  start_clamav

  echo "Starting Firebase Auth and Storage Emulators..."
  (
    cd "$BACKEND_DIR"
    exec env DEBUG= FIREBASE_SERVICE_ACCOUNT_JSON= GOOGLE_APPLICATION_CREDENTIALS= \
      "$FIREBASE_CLI" emulators:start --only auth,storage --project "$firebase_project" --config "$ROOT_DIR/firebase.json"
  ) &
  local firebase_pid=$!
  child_pids+=("$firebase_pid")
  wait_for_port "Firebase Auth Emulator" "$FIREBASE_AUTH_PORT" "$firebase_pid"
  wait_for_port "Firebase Storage Emulator" "$FIREBASE_STORAGE_PORT" "$firebase_pid"

  echo "Starting Axum backend..."
  (
    cd "$BACKEND_DIR"
    exec env \
      CLAMAV_ADDRESS="127.0.0.1:$clamav_port" \
      FIREBASE_AUTH_EMULATOR_HOST="127.0.0.1:$FIREBASE_AUTH_PORT" \
      FIREBASE_STORAGE_EMULATOR_HOST="127.0.0.1:$FIREBASE_STORAGE_PORT" \
      cargo run
  ) &
  local backend_pid=$!
  child_pids+=("$backend_pid")
  wait_for_port "Axum backend" "$BACKEND_PORT" "$backend_pid"

  echo "Starting Next.js frontend..."
  (
    cd "$FRONTEND_DIR"
    exec env NEXT_PUBLIC_FIREBASE_AUTH_EMULATOR_URL="http://127.0.0.1:$FIREBASE_AUTH_PORT" \
      bun run dev -- --port "$FRONTEND_PORT"
  ) &
  local frontend_pid=$!
  child_pids+=("$frontend_pid")
  wait_for_port "Next.js frontend" "$FRONTEND_PORT" "$frontend_pid"

  echo "Local stack is ready: http://127.0.0.1:$FRONTEND_PORT"
  echo "Press Ctrl-C to stop the Firebase emulators, backend, and frontend. PostgreSQL and the ClamAV signature volume remain running."

  while true; do
    local pid
    for pid in "${child_pids[@]}"; do
      if ! kill -0 "$pid" 2>/dev/null; then
        wait "$pid" || true
        fail "A local development service exited unexpectedly."
      fi
    done
    sleep 1
  done
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
