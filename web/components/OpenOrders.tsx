"use client";

import useSWR from "swr";
import { fetcher } from "@/lib/fetcher";

type OpenOrder = {
  order_id: string;
  instrument_id: number;
  side: string;
  order_type: string;
  price: number;
  qty: number;
  remaining: number;
  status: string;
};

type Props = {
  participant: string;
  symbol: string;
  onMutate?: () => void;
};

export default function OpenOrders({ participant, symbol, onMutate }: Props) {
  const { data, error, mutate } = useSWR<OpenOrder[]>(
    participant
      ? `/api/orders?participant=${encodeURIComponent(participant)}&instrument=${encodeURIComponent(symbol)}`
      : null,
    fetcher,
    { refreshInterval: 3000 }
  );

  async function handleCancel(orderId: string) {
    try {
      await fetch(`/api/order?id=${encodeURIComponent(orderId)}`, {
        method: "DELETE",
      });
      mutate();
      onMutate?.();
    } catch {
      /* ignore */
    }
  }

  if (!participant) {
    return <div className="empty-state px-4 text-sm">Set a trader ID to view orders.</div>;
  }

  if (error) return <div className="text-xs text-red-300">Failed to load</div>;
  if (!data) return <div className="empty-state px-4 text-sm">Loading orders...</div>;
  if (!Array.isArray(data)) return <div className="text-xs text-red-300">Invalid data</div>;

  return (
    <div className="font-mono text-xs">
      <div className="grid grid-cols-[3.5rem_1fr_1.35fr_2rem] px-2 pb-2 text-[0.68rem] uppercase tracking-wider text-neutral-600">
        <span>Side</span>
        <span className="text-right">Price</span>
        <span className="text-right">Qty/Filled</span>
        <span />
      </div>
      <div className="max-h-64 space-y-1 overflow-y-auto">
        {data.length === 0 && (
          <div className="empty-state px-4 text-sm">No open orders</div>
        )}
        {data.map((order) => {
          const filled = order.qty - order.remaining;
          return (
            <div
              key={order.order_id}
              className={`grid grid-cols-[3.5rem_1fr_1.35fr_2rem] items-center rounded-lg px-2 py-2 transition-colors ${
                order.side === "buy"
                  ? "bg-emerald-500/5 hover:bg-emerald-500/10"
                  : "bg-rose-500/5 hover:bg-rose-500/10"
              }`}
            >
              <span
                className={`font-semibold ${
                  order.side === "buy" ? "text-emerald-300" : "text-rose-300"
                }`}
              >
                {order.side.toUpperCase()}
              </span>
              <span className="text-right text-neutral-300">
                {order.price != null ? order.price.toFixed(2) : "MKT"}
              </span>
              <span className="text-right text-neutral-500">
                {order.qty.toFixed(4)}/{filled.toFixed(4)}
              </span>
              <button
                onClick={() => handleCancel(order.order_id)}
                className="ml-2 rounded-md py-1 text-neutral-600 transition-colors hover:bg-rose-500/10 hover:text-rose-300"
                title="Cancel order"
                aria-label="Cancel order"
              >
                x
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
