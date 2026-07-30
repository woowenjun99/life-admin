#!/usr/bin/env bash

set -Eeuo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# Sourcing defines the launcher helpers without starting services.
source "$SCRIPT_DIR/start-local.sh"

failure_message=""
kill_called=false

fail() {
  failure_message="$*"
  return 1
}

listener_pids() {
  printf '4242\n'
}

kill() {
  kill_called=true
}

if require_available_port 3000; then
  echo "Expected an occupied port to fail." >&2
  exit 1
fi

[[ "$failure_message" == *"Port 3000 is already in use by PID(s): 4242."* ]]
[[ "$kill_called" == false ]]
