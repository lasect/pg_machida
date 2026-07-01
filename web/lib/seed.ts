import { sql } from "drizzle-orm";
import { db } from "./db";

const INSTRUMENTS = [
  ["BTC-USD", 0.01, 1, 10000000],
  ["ETH-USD", 0.01, 1, 10000000],
  ["SOL-USD", 0.01, 1, 10000000],
  ["AVAX-USD", 0.01, 1, 10000000],
  ["LINK-USD", 0.01, 1, 10000000],
] as const;

const PARTICIPANTS = [
  ["trader_1", "Alice"],
  ["trader_2", "Bob"],
  ["market_maker", "LiquidityBot"],
] as const;

type LiquidityLevels = { bids: [number, number][]; asks: [number, number][] };

const LIQUIDITY: Record<string, LiquidityLevels> = {
  "BTC-USD": {
    bids: [
      [59800, 1],
      [59900, 2],
      [59950, 1],
      [60000, 3],
    ],
    asks: [
      [60100, 1],
      [60150, 2],
      [60200, 1],
      [60300, 3],
    ],
  },
  "ETH-USD": {
    bids: [
      [2980, 5],
      [2990, 10],
      [2995, 7],
      [3000, 20],
    ],
    asks: [
      [3010, 5],
      [3020, 10],
      [3025, 7],
      [3040, 20],
    ],
  },
  "SOL-USD": {
    bids: [
      [138, 50],
      [139, 100],
      [139.5, 75],
      [140, 200],
    ],
    asks: [
      [141, 50],
      [142, 100],
      [142.5, 75],
      [144, 200],
    ],
  },
  "AVAX-USD": {
    bids: [
      [24.5, 100],
      [24.75, 200],
      [24.9, 150],
      [25.0, 500],
    ],
    asks: [
      [25.1, 100],
      [25.25, 200],
      [25.4, 150],
      [25.5, 500],
    ],
  },
  "LINK-USD": {
    bids: [
      [12.8, 200],
      [12.9, 400],
      [12.95, 300],
      [13.0, 1000],
    ],
    asks: [
      [13.1, 200],
      [13.2, 400],
      [13.25, 300],
      [13.3, 1000],
    ],
  },
};

let engineSeeded = false;

export async function ensureEngine() {
  if (engineSeeded) return;

  try {
    const result = await db.execute(
      sql`SELECT * FROM clob.get_open_orders('market_maker', null) LIMIT 1`
    );
    if (result.length > 0) {
      engineSeeded = true;
      return;
    }
  } catch (err) {
    if (engineSeeded) {
      console.error("ensureEngine: query failed but engine is marked as seeded, skipping re-seed", err);
      return;
    }
    console.error("ensureEngine: engine not initialised, will seed", err);
  }

  // Clear PG tables (they may have stale rows from a previous failed seed)
  await db.execute(sql`TRUNCATE clob.instruments CASCADE`);
  await db.execute(sql`TRUNCATE clob.participants CASCADE`);

  // ── Instruments ─────────────────────────────────────────────
  for (const [sym, tick, lot, max] of INSTRUMENTS) {
    await db.execute(
      sql`SELECT clob.create_instrument(${sym}, ${tick}::numeric, ${lot}::numeric, ${max})`
    );
  }

  // ── Participants ───────────────────────────────────────────
  for (const [id, name] of PARTICIPANTS) {
    await db.execute(sql`SELECT clob.create_participant(${id}, ${name})`);
  }

  // ── Liquidity (resting limit orders) ───────────────────────
  for (const [instrument, levels] of Object.entries(LIQUIDITY)) {
    for (const [price, qty] of levels.bids) {
      await db.execute(
        sql`SELECT * FROM clob.place_order(${instrument}, 'buy', 'limit', 'market_maker', ${qty}::numeric, ${price}::numeric, 'cancel_newest')`
      );
    }
    for (const [price, qty] of levels.asks) {
      await db.execute(
        sql`SELECT * FROM clob.place_order(${instrument}, 'sell', 'limit', 'market_maker', ${qty}::numeric, ${price}::numeric, 'cancel_newest')`
      );
    }
  }
}
