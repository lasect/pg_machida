"use client";

import useSWR from "swr";
import { fetcher } from "@/lib/fetcher";

type Trade = {
  id: string;
  price: string;
  qty: string;
  buy_participant_id: string;
  sell_participant_id: string;
  executed_at: string;
};

export default function TradeTape({ symbol }: { symbol: string }) {
  const { data, error } = useSWR<Trade[]>(
    symbol
      ? `/api/trades?symbol=${encodeURIComponent(symbol)}&limit=30`
      : null,
    fetcher,
    { refreshInterval: 2000 }
  );

  if (!symbol) return <div className="empty-state px-4 text-sm">Select an instrument to watch prints.</div>;

  if (error) return <div className="text-xs text-red-300">Failed to load</div>;
  if (!data) return <div className="empty-state px-4 text-sm">Loading trades...</div>;
  if (!Array.isArray(data)) return <div className="text-xs text-red-300">Invalid data</div>;

  return (
    <div className="font-mono text-xs">
      <div className="grid grid-cols-3 px-2 pb-2 text-[0.68rem] uppercase tracking-wider text-neutral-600">
        <span>Price</span>
        <span className="text-right">Qty</span>
        <span className="text-right">Time</span>
      </div>
      <div className="max-h-[28rem] space-y-px overflow-y-auto">
        {data.length === 0 && (
          <div className="empty-state px-4 text-sm">No trades yet</div>
        )}
        {data.map((trade) => {
          const numPrice = parseFloat(trade.price);
          const numQty = parseFloat(trade.qty);
          const time = new Date(trade.executed_at).toLocaleTimeString(
            "en-US",
            { hour12: false }
          );
          return (
            <div
              key={trade.id}
              className="grid grid-cols-3 rounded-md px-2 py-1.5 transition-colors hover:bg-neutral-800/55"
            >
              <span className="text-neutral-200">{numPrice.toFixed(2)}</span>
              <span className="text-right text-neutral-400">{numQty.toFixed(4)}</span>
              <span className="text-right text-neutral-600">{time}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
