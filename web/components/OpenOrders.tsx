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
    return <div className="text-neutral-500 text-sm">Set a trader ID</div>;
  }

  if (error) return <div className="text-red-400 text-xs">Failed to load</div>;
  if (!data) return <div className="text-neutral-500 text-xs">Loading orders...</div>;
  if (!Array.isArray(data)) return <div className="text-red-400 text-xs">Invalid data</div>;

  return (
    <div className="font-mono text-xs">
      <div className="flex justify-between text-neutral-500 mb-1 px-1">
        <span>Side</span>
        <span>Price</span>
        <span>Qty/Filled</span>
        <span />
      </div>
      <div className="space-y-px max-h-48 overflow-y-auto">
        {data.length === 0 && (
          <div className="text-neutral-600 px-1">No open orders</div>
        )}
        {data.map((order) => {
          const filled = order.qty - order.remaining;
          return (
            <div
              key={order.order_id}
              className={`flex items-center justify-between px-1 py-1 rounded ${
                order.side === "buy"
                  ? "bg-green-500/5 hover:bg-green-500/10"
                  : "bg-red-500/5 hover:bg-red-500/10"
              }`}
            >
              <span
                className={`w-8 ${
                  order.side === "buy" ? "text-green-400" : "text-red-400"
                }`}
              >
                {order.side.toUpperCase()}
              </span>
              <span className="text-neutral-300">
                {order.price != null ? order.price.toFixed(2) : "MKT"}
              </span>
              <span className="text-neutral-400">
                {order.qty.toFixed(4)}/{filled.toFixed(4)}
              </span>
              <button
                onClick={() => handleCancel(order.order_id)}
                className="text-neutral-600 hover:text-red-400 transition-colors ml-2"
                title="Cancel order"
              >
                ✕
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
