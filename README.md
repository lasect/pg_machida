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
- **Idempotent trade IDs** (UUIDv5) for duplicate-safe trade inserts
- **Rebuild helpers** for restoring in-memory books from persisted fixtures/rows
- **100+ tests** across unit and pgrx SQL suites

Comes with a **Next.js 16 trading UI** (`web/`) for placing orders, viewing the order book, trade tape, and open orders in the browser.

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
  │  │  persistence.rs — rebuild helpers         │  │
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
  │  Next.js 16 Web App (web/)                    │
  │  app/api/*    — REST API routes               │
  │  lib/queries.ts — SQL calls to clob.*         │
  │  components/* — OrderBook, TradeTape, etc.    │
  └───────────────────────────────────────────────┘
```

The engine is stored as a process-local static singleton (`OnceLock<Mutex<ClobEngine>>`) inside the extension. PostgreSQL uses separate backend processes, so true cross-backend shared state is future work and is tracked below under shared memory (DSM). The order book uses a hybrid design: a flat tick array for aggregate quantity per level, plus per-level `VecDeque<Order>` queues for FIFO order identity and cancellation by ID.

### Project layout

| Directory | |
|---|---|
| `src/` | Rust extension — `lib.rs` (SQL bindings), `engine.rs`, `book.rs`, `matching.rs`, `persistence.rs`, `types.rs` |
| `sql/` | SQL migration files defining the `clob` schema tables |
| `tests/` | Unit tests (`unit/`) per module + integration tests (`integration/test_sql.rs`) |
| `web/` | Next.js 16 App Router UI — components for order book, trade tape, open orders; API routes under `app/api/`; `lib/queries.ts` for SQL calls |

---

## SQL API

All functions live in the `clob` schema:

| Function | Description |
|----------|-------------|
| `clob.create_instrument(symbol, tick_size, lot_size, max_ticks)` | Register a new trading instrument |
| `clob.create_participant(participant_id, display_name)` | Register or update a trader |
| `clob.place_order(instrument, side, order_type, participant, qty, price, stp_mode)` | Place an order |
| `clob.cancel_order(order_id)` | Cancel a resting order |
| `clob.get_book(instrument, depth)` | Get the current in-memory order book |
| `clob.get_trades(instrument, limit)` | Get recent persisted trades |
| `clob.get_open_orders(participant, instrument)` | Get open orders for a participant |
| `clob.mass_cancel(participant, instrument)` | Cancel all orders for a participant on an instrument |
| `clob.halt_instrument(instrument)` / `clob.resume_instrument(instrument)` | Halt/resume trading |
| `clob.snapshot_book(instrument)` | Persist current book depth to `clob.book_snapshots` |

### Example

```sql
SELECT clob.create_instrument('BTC-USD', 0.01, 1, 10000000);
SELECT clob.create_participant('alice', 'Alice');

-- Alice places a limit buy for 1.0 BTC at $60,000
SELECT * FROM clob.place_order(
    'BTC-USD',
    'buy',
    'limit',
    'alice',
    1.0,
    60000,
    'cancel_newest'
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
pnpm install
DATABASE_URL=postgres://USER@localhost:28816/pg_machida_dev pnpm run dev
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

The core matching engine is implemented and covered by unit and pgrx SQL tests. The current extension keeps order-book state in memory per PostgreSQL backend process; production-grade shared state, startup recovery, and streaming are still roadmap items.

Current limitations:

- Normal order placement, fills, cancels, and mass cancels are persisted to `clob.orders`, but automatic startup rebuild from those rows is not wired into the extension yet.
- STP modes that remove/decrement resting orders need richer engine events before their database state can be fully persisted.
- Price indexing currently uses cent ticks internally, so configurable tick sizes need more hardening.

Roadmap:

- [ ] Background worker for async trade persistence
- [ ] Shared memory (DSM) for true multi-backend concurrency
- [ ] `LISTEN`/`NOTIFY` for real-time streaming
- [ ] Iceberg orders
- [ ] Risk limits (VaR, position checks)
- [ ] Production hardening

---

## License

MIT
