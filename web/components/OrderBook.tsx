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
    return <div className="text-neutral-500 text-sm">Select an instrument</div>;
  }

  if (error) return <div className="text-red-400 text-sm">Failed to load</div>;
  if (!data) return <div className="text-neutral-500 text-sm">Loading book...</div>;
  if (!Array.isArray(data)) return <div className="text-red-400 text-sm">Invalid data</div>;

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
      <div className="flex justify-between text-neutral-500 mb-1 px-1">
        <span>Price</span>
        <span>Qty</span>
      </div>

      {/* Asks (sell side) - reverse so best is at bottom */}
      <div className="space-y-px">
        {asks
          .slice()
          .reverse()
          .map((level, i) => (
            <div
              key={`ask-${level.price}-${i}`}
              className="flex justify-between items-center px-1 py-0.5 relative"
            >
              <div
                className="absolute inset-0 bg-red-500/10"
                style={{ width: `${(level.qty / maxQty) * 100}%` }}
              />
              <span className="relative z-10 text-red-400">{level.price}</span>
              <span className="relative z-10 text-red-300">{level.qty}</span>
            </div>
          ))}
      </div>

      {/* Spread */}
      <div className="flex justify-between items-center py-1.5 px-1 border-y border-neutral-800 my-1">
        <span className="text-neutral-300 font-semibold text-sm">
          {bestBid || "—"} / {bestAsk || "—"}
        </span>
        <span className="text-neutral-500">
          Spread: {spread.toFixed(2)}
        </span>
      </div>

      {/* Bids (buy side) */}
      <div className="space-y-px">
        {bids.map((level, i) => (
          <div
            key={`bid-${level.price}-${i}`}
            className="flex justify-between items-center px-1 py-0.5 relative"
          >
            <div
              className="absolute inset-0 bg-green-500/10"
              style={{ width: `${(level.qty / maxQty) * 100}%` }}
            />
            <span className="relative z-10 text-green-400">{level.price}</span>
            <span className="relative z-10 text-green-300">{level.qty}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
