import { sql } from "drizzle-orm";
import { db } from "./db";
import { instruments, trades } from "./schema";
import { ensureEngine } from "./seed";

// ── Instruments ───────────────────────────────────────────────

export async function getInstruments() {
  return db.select().from(instruments).orderBy(instruments.symbol);
}

// ── Order Book ────────────────────────────────────────────────

export async function getBook(symbol: string, depth: number = 10) {
  await ensureEngine();
  return db.execute(
    sql`SELECT * FROM clob.get_book(${symbol}, ${depth})`
  );
}

// ── Place Order ───────────────────────────────────────────────

export async function placeOrder(params: {
  instrument: string;
  side: string;
  orderType: string;
  qty: number;
  participant: string;
  price?: number;
  stpMode?: string;
}) {
  await ensureEngine();
  const result = await db.execute(
    sql`SELECT * FROM clob.place_order(
      ${params.instrument},
      ${params.side},
      ${params.orderType},
      ${params.participant},
      ${params.qty}::numeric,
      ${params.price ?? null}::numeric,
      ${params.stpMode ?? "cancel_newest"}
    )`
  );
  const rows = result as unknown as Array<{
    order_id: string;
    status: string;
    filled_qty: number;
    avg_price: number;
  }>;
  return rows[0];
}

// ── Cancel Order ──────────────────────────────────────────────

export async function cancelOrder(orderId: string) {
  await ensureEngine();
  const result = await db.execute(
    sql`SELECT * FROM clob.cancel_order(${orderId}::uuid)`
  );
  const rows = result as unknown as Array<{ cancel_order: boolean }>;
  return rows[0];
}

// ── Open Orders ───────────────────────────────────────────────

export async function getOpenOrders(
  participant: string,
  instrument?: string | null
) {
  await ensureEngine();
  return db.execute(
    sql`SELECT * FROM clob.get_open_orders(${participant}, ${instrument ?? null})`
  );
}

// ── Trades ────────────────────────────────────────────────────

export async function getTrades(symbol: string, limit: number = 50) {
  await ensureEngine();
  return db.execute(
    sql`SELECT * FROM clob.get_trades(${symbol}, ${limit})`
  );
}

// ── Create Participant ────────────────────────────────────────

export async function createParticipant(id: string, displayName?: string) {
  return db.execute(
    sql`SELECT clob.create_participant(${id}, ${displayName ?? null})`
  );
}
