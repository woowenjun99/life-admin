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

release_postgres_port() {
  local service_id

  service_id="$(docker compose --project-directory "$ROOT_DIR" ps --quiet postgres)"
  if [[ -n "$service_id" ]]; then
    echo "Stopping this project's existing PostgreSQL container..."
    docker compose --project-directory "$ROOT_DIR" stop postgres >/dev/null
  fi

  require_available_port "$POSTGRES_PORT"
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

  trap cleanup EXIT INT TERM

  require_available_port "$FRONTEND_PORT"
  require_available_port "$BACKEND_PORT"
  require_available_port "$FIREBASE_UI_PORT"
  require_available_port "$FIREBASE_AUTH_PORT"
  release_postgres_port

  reset_database

  echo "Starting Firebase Auth Emulator..."
  (
    cd "$BACKEND_DIR"
    exec env DEBUG= FIREBASE_SERVICE_ACCOUNT_JSON= GOOGLE_APPLICATION_CREDENTIALS= \
      "$FIREBASE_CLI" emulators:start --only auth --project "$firebase_project" --config "$ROOT_DIR/firebase.json"
  ) &
  local firebase_pid=$!
  child_pids+=("$firebase_pid")
  wait_for_port "Firebase Auth Emulator" "$FIREBASE_AUTH_PORT" "$firebase_pid"

  echo "Starting Axum backend..."
  (
    cd "$BACKEND_DIR"
    exec env FIREBASE_AUTH_EMULATOR_HOST="127.0.0.1:$FIREBASE_AUTH_PORT" cargo run
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
  echo "Press Ctrl-C to stop the Firebase emulator, backend, and frontend. PostgreSQL remains running."

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
