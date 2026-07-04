"use client";

import useSWR from "swr";
import { fetcher } from "@/lib/fetcher";

type BookLevel = {
  side: string;
  price: number;
  qty: number;
  order_count: number;
};

export default function OrderBook({ symbol }: { symbol: string }) {
  const { data, error } = useSWR<BookLevel[]>(
    symbol ? `/api/book?symbol=${encodeURIComponent(symbol)}&depth=10` : null,
    fetcher,
    { refreshInterval: 2000 }
  );

  if (!symbol) {
    return <div className="empty-state px-4 text-sm">Select an instrument to view live depth.</div>;
  }

  if (error) return <div className="text-sm text-red-300">Failed to load</div>;
  if (!data) return <div className="empty-state px-4 text-sm">Loading book...</div>;
  if (!Array.isArray(data)) return <div className="text-sm text-red-300">Invalid data</div>;

  const asks = data
    .filter((l) => l.side === "sell")
    .sort((a, b) => a.price - b.price);
  const bids = data
    .filter((l) => l.side === "buy")
    .sort((a, b) => b.price - a.price);

  const bestBid = bids[0]?.price ?? 0;
  const bestAsk = asks[0]?.price ?? 0;
  const spread = bestAsk && bestBid ? bestAsk - bestBid : 0;

  const maxQty = Math.max(
    ...bids.map((l) => l.qty),
    ...asks.map((l) => l.qty),
    1
  );

  return (
    <div className="font-mono text-xs">
      <div className="grid grid-cols-3 px-2 pb-2 text-[0.68rem] uppercase tracking-wider text-neutral-600">
        <span>Price</span>
        <span className="text-right">Qty</span>
        <span className="text-right">Orders</span>
      </div>

      <div className="space-y-px">
        {asks
          .slice()
          .reverse()
          .map((level, i) => (
            <div
              key={`ask-${level.price}-${i}`}
              className="relative grid grid-cols-3 items-center overflow-hidden rounded-md px-2 py-1.5"
            >
              <div
                className="absolute inset-y-0 right-0 bg-rose-500/10"
                style={{ width: `${(level.qty / maxQty) * 100}%` }}
              />
              <span className="relative z-10 text-rose-300">{level.price.toFixed(2)}</span>
              <span className="relative z-10 text-right text-rose-100/80">{level.qty.toFixed(4)}</span>
              <span className="relative z-10 text-right text-neutral-600">{level.order_count}</span>
            </div>
          ))}
      </div>

      <div className="my-2 flex items-center justify-between rounded-xl border border-neutral-800 bg-neutral-950/70 px-3 py-2">
        <span className="text-sm font-semibold text-neutral-100">
          {bestBid || "—"} / {bestAsk || "—"}
        </span>
        <span className="text-neutral-500">
          Spread: {spread.toFixed(2)}
        </span>
      </div>

      <div className="space-y-px">
        {bids.map((level, i) => (
          <div
            key={`bid-${level.price}-${i}`}
            className="relative grid grid-cols-3 items-center overflow-hidden rounded-md px-2 py-1.5"
          >
            <div
              className="absolute inset-y-0 right-0 bg-emerald-500/10"
              style={{ width: `${(level.qty / maxQty) * 100}%` }}
            />
            <span className="relative z-10 text-emerald-300">{level.price.toFixed(2)}</span>
            <span className="relative z-10 text-right text-emerald-100/80">{level.qty.toFixed(4)}</span>
            <span className="relative z-10 text-right text-neutral-600">{level.order_count}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
