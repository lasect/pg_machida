# pg_machida

**A CLOB stock exchange matching engine embedded inside PostgreSQL.**

pg_machida is a PostgreSQL extension (written in Rust with [pgrx](https://github.com/pgcentralfoundation/pgrx)) that runs a full central limit order book (CLOB) matching engine directly inside the database process. Place, cancel, and query orders using SQL functions — no external services, no network hops.

---

## Features

- **Price-time priority CLOB matching** — standard FIFO crossing logic
- **Limit, Market, IOC, and FOK** order types
- **Self-trade prevention (STP)** — CancelNewest, CancelOldest, Decrement, or None
- **Multi-instrument support** — each instrument has an isolated order book
- **Tick-size and lot-size validation** per instrument
- **Circuit breakers** — halt an instrument when a trade breaches a threshold
- **Mass cancel** — cancel all orders for a participant on an instrument
- **Instrument halt/resume**
- **Idempotent trade IDs** (UUIDv5) for safe crash recovery
- **Book persistence** — rebuild in-memory state from Postgres tables on startup
- **~116 tests** across unit and integration suites

Comes with a **Next.js 15 trading UI** (`web/`) for placing orders, viewing the order book, trade tape, and open orders in the browser.

---

## Architecture

```
  ┌────────────────────────────────────────────────┐
  │        PostgreSQL (pgrx-managed PG16)           │
  │                                                 │
  │  ┌──────────────────────────────────────────┐  │
  │  │  pg_machida Extension (Rust .so)         │  │
  │  │  lib.rs      — pg_extern SQL functions   │  │
  │  │  state.rs    — global engine singleton    │  │
  │  │  engine.rs   — multi-instrument manager   │  │
  │  │  book.rs     — tick array + FIFO queues   │  │
  │  │  matching.rs — core matching loop         │  │
  │  │  types.rs    — Order, Trade, Side, etc.   │  │
  │  │  persistence.rs — rebuild from DB tables  │  │
  │  └──────────────────────────────────────────┘  │
  │                                                 │
  │  ┌──────────────────────────────────────────┐  │
  │  │  clob schema tables                      │  │
  │  │  instruments, participants, orders,      │  │
  │  │  trades, book_snapshots                  │  │
  │  └──────────────────────────────────────────┘  │
  └────────────────────────────────────────────────┘
                        │ SQL via postgres.js
  ┌─────────────────────▼─────────────────────────┐
  │  Next.js 15 Web App (web/)                    │
  │  app/api/*    — REST API routes               │
  │  lib/queries.ts — SQL calls to clob.*         │
  │  components/* — OrderBook, TradeTape, etc.    │
  └───────────────────────────────────────────────┘
```

The engine is stored as a static singleton (`OnceLock<Mutex<ClobEngine>>`), shared across all PostgreSQL backends. The order book uses a hybrid design: a flat tick array for O(1) aggregate quantity per level, plus per-level `VecDeque<Order>` queues for FIFO order identity and O(1) cancellation by ID.

### Project layout

| Directory | |
|---|---|
| `src/` | Rust extension — `lib.rs` (SQL bindings), `engine.rs`, `book.rs`, `matching.rs`, `persistence.rs`, `types.rs`, plus background worker and notify modules |
| `sql/` | SQL migration files defining the `clob` schema tables |
| `tests/` | Unit tests (`unit/`) per module + integration tests (`integration/test_sql.rs`) |
| `web/` | Next.js 16 App Router UI — components for order book, trade tape, open orders; API routes under `app/api/`; `lib/queries.ts` for SQL calls |
| `pg_machida.md` | Design doc and roadmap |

---

## SQL API

All functions live in the `clob` schema:

| Function | Description |
|----------|-------------|
| `clob.create_instrument(symbol, name, tick_size, lot_size, price_precision, qty_precision)` | Register a new trading instrument |
| `clob.create_participant(name, participant_id)` | Register a trader |
| `clob.place_order(participant_id, symbol, side, order_type, price, qty, stp, instrument_id)` | Place an order (returns trades and order status) |
| `clob.cancel_order(participant_id, symbol, order_id)` | Cancel a resting order |
| `clob.get_book(symbol, depth)` | Get the current order book |
| `clob.get_trades(symbol, limit)` | Get recent trades |
| `clob.get_open_orders(participant_id, symbol)` | Get open orders for a participant |
| `clob.mass_cancel(participant_id, symbol)` | Cancel all orders for a participant on an instrument |
| `clob.halt_instrument(symbol)` / `clob.resume_instrument(symbol)` | Halt/resume trading |
| `clob.snapshot_book(symbol)` | Persist current book state to `clob.book_snapshots` |

### Example

```sql
SELECT * FROM clob.create_instrument('BTC-USD', 'Bitcoin', 0.01, 1, 2, 8);
SELECT * FROM clob.create_participant('alice', 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11');

-- Alice places a limit buy for 1.0 BTC at $60,000
SELECT * FROM clob.place_order(
    'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11', 'BTC-USD',
    'Buy', 'Limit', 60000, 1.0,
    'None',
    (SELECT id FROM clob.instruments WHERE symbol = 'BTC-USD')
);

-- Check the book
SELECT * FROM clob.get_book('BTC-USD', 10);
```

---

## Requirements

- **Rust** (latest stable, edition 2021)
- **PostgreSQL 16** (also supports PG 13–18 via pgrx features)
- **cargo-pgrx 0.18.0**
- **Node.js 20+** and **pnpm** (for the web UI)

---

## Quick Start

### 1. Install pgrx and the extension

```bash
# Install the pgrx CLI
cargo install cargo-pgrx --version 0.18.0

# Initialize pgrx for PG16
cargo pgrx init --pg16 /usr/lib/postgresql/16/bin/pg_config

# Build and install the extension
cargo pgrx install

# Start the pgrx-managed PostgreSQL
cargo pgrx start pg16
```

### 2. Create the extension in your database

```sql
CREATE DATABASE pg_machida_dev;
\c pg_machida_dev
CREATE EXTENSION pg_machida;
```

### 3. (Optional) Start the trading web UI

```bash
cd web
cp .env.local.example .env.local  # or set DATABASE_URL
pnpm install
pnpm run dev
# Open http://localhost:3000
```

Or use the one-command dev script:

```bash
./dev.sh
```

---

## Running Tests

```bash
# Unit tests (no PostgreSQL needed)
cargo test

# Integration tests (requires a running PG16 instance)
cargo pgrx test pg16
```

---

## Project Status

The core matching engine is fully implemented and extensively tested. Remaining work (tracked in `pg_machida.md`):

- [ ] Background worker for async trade persistence
- [ ] Shared memory (DSM) for true multi-backend concurrency
- [ ] `LISTEN`/`NOTIFY` for real-time streaming
- [ ] Iceberg orders
- [ ] Risk limits (VaR, position checks)
- [ ] Production hardening

---

## License

MIT
