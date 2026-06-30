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

  if (!symbol) return null;

  if (error) return <div className="text-red-400 text-xs">Failed to load</div>;
  if (!data) return <div className="text-neutral-500 text-xs">Loading trades...</div>;
  if (!Array.isArray(data)) return <div className="text-red-400 text-xs">Invalid data</div>;

  return (
    <div className="font-mono text-xs">
      <div className="flex justify-between text-neutral-500 mb-1 px-1">
        <span>Price</span>
        <span>Qty</span>
        <span>Time</span>
      </div>
      <div className="space-y-px max-h-64 overflow-y-auto">
        {data.length === 0 && (
          <div className="text-neutral-600 px-1">No trades yet</div>
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
              className="flex justify-between px-1 py-0.5 hover:bg-neutral-800/50"
            >
              <span className="text-neutral-300">{numPrice.toFixed(2)}</span>
              <span className="text-neutral-400">{numQty.toFixed(4)}</span>
              <span className="text-neutral-600">{time}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
