import { sql } from "drizzle-orm";
import {
  pgSchema,
  bigserial,
  bigint,
  text,
  numeric,
  integer,
  timestamp,
  uuid,
  index,
} from "drizzle-orm/pg-core";

export const clobSchema = pgSchema("clob");

export const instruments = clobSchema.table("instruments", {
  id: bigserial("id", { mode: "number" }).primaryKey(),
  symbol: text("symbol").notNull().unique(),
  tickSize: numeric("tick_size").notNull().default("0.01"),
  lotSize: numeric("lot_size").notNull().default("1"),
  maxTicks: integer("max_ticks").notNull().default(10000000),
  status: text("status").notNull().default("active"),
  createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
});

export const participants = clobSchema.table("participants", {
  id: text("id").primaryKey(),
  displayName: text("display_name"),
  createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
});

export const orders = clobSchema.table(
  "orders",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    instrumentId: bigint("instrument_id", { mode: "number" })
      .notNull()
      .references(() => instruments.id),
    participantId: text("participant_id")
      .notNull()
      .references(() => participants.id),
    side: text("side").notNull(),
    orderType: text("order_type").notNull(),
    price: numeric("price"),
    qty: numeric("qty").notNull(),
    remaining: numeric("remaining").notNull(),
    status: text("status").notNull().default("open"),
    stpMode: text("stp_mode").notNull().default("cancel_newest"),
    createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true }).notNull().defaultNow(),
  },
  (table) => [
    index("orders_buy_book_idx")
      .on(table.instrumentId, table.price.desc(), table.createdAt.asc())
      .where(
        sql`${table.side} = 'buy' AND ${table.status} IN ('open', 'partially_filled')`
      ),
    index("orders_sell_book_idx")
      .on(table.instrumentId, table.price.asc(), table.createdAt.asc())
      .where(
        sql`${table.side} = 'sell' AND ${table.status} IN ('open', 'partially_filled')`
      ),
    index("orders_participant_idx").on(table.participantId),
  ]
);

export const trades = clobSchema.table(
  "trades",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    instrumentId: bigint("instrument_id", { mode: "number" })
      .notNull()
      .references(() => instruments.id),
    buyOrderId: uuid("buy_order_id").notNull(),
    sellOrderId: uuid("sell_order_id").notNull(),
    buyParticipantId: text("buy_participant_id").notNull(),
    sellParticipantId: text("sell_participant_id").notNull(),
    price: numeric("price").notNull(),
    qty: numeric("qty").notNull(),
    executedAt: timestamp("executed_at", { withTimezone: true }).notNull().defaultNow(),
  },
  (table) => [
    index("trades_instrument_time_idx").on(
      table.instrumentId,
      table.executedAt.desc()
    ),
    index("trades_buy_participant_idx").on(table.buyParticipantId),
    index("trades_sell_participant_idx").on(table.sellParticipantId),
  ]
);

export const bookSnapshots = clobSchema.table("book_snapshots", {
  id: bigserial("id", { mode: "number" }).primaryKey(),
  instrumentId: bigint("instrument_id", { mode: "number" })
    .notNull()
    .references(() => instruments.id),
  side: text("side").notNull(),
  price: numeric("price").notNull(),
  qty: numeric("qty").notNull(),
  orderCount: integer("order_count").notNull(),
  snapshotAt: timestamp("snapshot_at", { withTimezone: true }).notNull().defaultNow(),
});
