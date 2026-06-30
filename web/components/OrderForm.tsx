"use client";

import { useState } from "react";

type Props = {
  symbol: string;
  participant: string;
  onOrderPlaced: () => void;
};

export default function OrderForm({ symbol, participant, onOrderPlaced }: Props) {
  const [side, setSide] = useState<"buy" | "sell">("buy");
  const [orderType, setOrderType] = useState<"limit" | "market">("limit");
  const [price, setPrice] = useState("");
  const [qty, setQty] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    setStatus(null);

    const body: any = {
      instrument: symbol,
      side,
      orderType,
      qty: parseFloat(qty),
      participant,
    };

    if (orderType === "limit") {
      body.price = parseFloat(price);
    }

    try {
      const res = await fetch("/api/order", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const data = await res.json();

      if (!res.ok) {
        setStatus(data.error ?? "Order failed");
      } else {
        setStatus(
          `Order ${data.order_id?.slice(0, 8)}... — ${data.status} (filled ${data.filled_qty} @ ${data.avg_price ?? "—"})`
        );
        setPrice("");
        setQty("");
        onOrderPlaced();
      }
    } catch {
      setStatus("Network error");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => setSide("buy")}
          className={`flex-1 py-1.5 text-sm font-medium rounded transition-colors ${
            side === "buy"
              ? "bg-green-600 text-white"
              : "bg-neutral-800 text-neutral-400 border border-neutral-700 hover:border-green-700"
          }`}
        >
          Buy
        </button>
        <button
          type="button"
          onClick={() => setSide("sell")}
          className={`flex-1 py-1.5 text-sm font-medium rounded transition-colors ${
            side === "sell"
              ? "bg-red-600 text-white"
              : "bg-neutral-800 text-neutral-400 border border-neutral-700 hover:border-red-700"
          }`}
        >
          Sell
        </button>
      </div>

      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => setOrderType("limit")}
          className={`flex-1 py-1 text-xs rounded border transition-colors ${
            orderType === "limit"
              ? "bg-neutral-700 border-neutral-500 text-white"
              : "bg-neutral-800 border-neutral-700 text-neutral-400"
          }`}
        >
          Limit
        </button>
        <button
          type="button"
          onClick={() => setOrderType("market")}
          className={`flex-1 py-1 text-xs rounded border transition-colors ${
            orderType === "market"
              ? "bg-neutral-700 border-neutral-500 text-white"
              : "bg-neutral-800 border-neutral-700 text-neutral-400"
          }`}
        >
          Market
        </button>
      </div>

      {orderType === "limit" && (
        <div>
          <label className="text-xs text-neutral-400 block mb-0.5">Price</label>
          <input
            type="number"
            step="0.01"
            min="0"
            value={price}
            onChange={(e) => setPrice(e.target.value)}
            placeholder="0.00"
            required
            className="w-full bg-neutral-800 border border-neutral-700 rounded px-3 py-1.5 text-sm text-white font-mono placeholder:text-neutral-600 focus:outline-none focus:border-blue-500"
          />
        </div>
      )}

      <div>
        <label className="text-xs text-neutral-400 block mb-0.5">Quantity</label>
        <input
          type="number"
          step="0.01"
          min="0"
          value={qty}
          onChange={(e) => setQty(e.target.value)}
          placeholder="0.00"
          required
          className="w-full bg-neutral-800 border border-neutral-700 rounded px-3 py-1.5 text-sm text-white font-mono placeholder:text-neutral-600 focus:outline-none focus:border-blue-500"
        />
      </div>

      <button
        type="submit"
        disabled={submitting}
        className={`w-full py-2 text-sm font-medium rounded transition-colors ${
          side === "buy"
            ? "bg-green-600 hover:bg-green-700 text-white disabled:opacity-50"
            : "bg-red-600 hover:bg-red-700 text-white disabled:opacity-50"
        }`}
      >
        {submitting
          ? "Placing..."
          : side === "buy"
            ? `Buy ${symbol}`
            : `Sell ${symbol}`}
      </button>

      {status && (
        <div className="text-xs text-neutral-400 font-mono break-all">
          {status}
        </div>
      )}
    </form>
  );
}
