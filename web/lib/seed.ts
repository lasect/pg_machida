import { sql } from "drizzle-orm";
import { db } from "./db";

export async function ensureEngine() {
  // Check if engine already has instruments
  try {
    await db.execute(sql`SELECT * FROM clob.get_book('BTC-USD', 1)`);
    return; // engine has state, nothing to do
  } catch {
    // engine is empty, need to seed
  }

  await db.execute(sql`TRUNCATE clob.instruments CASCADE`);
  await db.execute(sql`TRUNCATE clob.participants CASCADE`);

  await db.execute(
    sql`SELECT clob.create_instrument('BTC-USD', 0.01, 1, 10000000)`
  );
  await db.execute(
    sql`SELECT clob.create_instrument('ETH-USD', 0.01, 1, 10000000)`
  );

  await db.execute(
    sql`SELECT clob.create_participant('trader_1', 'Alice')`
  );
  await db.execute(
    sql`SELECT clob.create_participant('trader_2', 'Bob')`
  );
}
