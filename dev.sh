#!/usr/bin/env bash
set -e

echo "=== Starting pg_machida local dev ==="

# 1. Build and install the Rust extension (if not already)
if [ ! -f target/release/libpg_machida.so ]; then
  echo "[1/3] Building extension..."
  cargo pgrx install --pg-config /home/deep/.pgrx/16.14/pgrx-install/bin/pg_config
fi

# 2. Start the pgrx-managed PostgreSQL
echo "[2/3] Starting PostgreSQL on port 28816..."
cargo pgrx start pg16 2>/dev/null || true

# 3. Start the Next.js dev server
echo "[3/3] Starting Next.js on http://localhost:3000"
cd web
pnpm run dev
