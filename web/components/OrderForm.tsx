"use client";

import { useState } from "react";

type Props = {
  symbol: string;
  participant: string;
  onOrderPlaced: () => void;
};

type OrderType = "limit" | "market" | "ioc" | "fok";

type OrderPayload = {
  instrument: string;
  side: "buy" | "sell";
  orderType: OrderType;
  qty: number;
  participant: string;
  price?: number;
};

const MAX_PRICE = 1_000_000;
const MAX_QTY = 1_000_000;
const MAX_DECIMAL_PLACES = 2;
const DECIMAL_INPUT_PATTERN = "[0-9]*[.]?[0-9]{0,2}";
const ORDER_TYPES: Array<{ value: OrderType; label: string }> = [
  { value: "limit", label: "Limit" },
  { value: "market", label: "Market" },
  { value: "ioc", label: "IOC" },
  { value: "fok", label: "FOK" },
];

function cleanDecimalInput(value: string, max: number) {
  const normalized = value.replace(/[^0-9.]/g, "");
  const [whole = "", ...rest] = normalized.split(".");
  const decimal = rest.join("").slice(0, MAX_DECIMAL_PLACES);
  const cleaned = rest.length > 0 ? `${whole}.${decimal}` : whole;

  if (Number(cleaned) > max) return max.toString();
  return cleaned;
}

function getAmountError(label: string, value: string, max: number) {
  const amount = Number(value);

  if (!Number.isFinite(amount) || amount <= 0) return `${label} must be greater than 0`;
  if (amount > max) return `${label} cannot exceed ${max.toLocaleString()}`;
  return null;
}

export default function OrderForm({ symbol, participant, onOrderPlaced }: Props) {
  const [side, setSide] = useState<"buy" | "sell">("buy");
  const [orderType, setOrderType] = useState<OrderType>("limit");
  const [price, setPrice] = useState("");
  const [qty, setQty] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const requiresPrice = orderType !== "market";

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const qtyError = getAmountError("Quantity", qty, MAX_QTY);
    const priceError =
      requiresPrice ? getAmountError("Price", price, MAX_PRICE) : null;

    if (!participant) {
      setStatus("Trader ID is required");
      return;
    }

    if (qtyError ?? priceError) {
      setStatus(qtyError ?? priceError);
      return;
    }

    setSubmitting(true);
    setStatus(null);

    const body: OrderPayload = {
      instrument: symbol,
      side,
      orderType,
      qty: parseFloat(qty),
      participant,
    };

    if (requiresPrice) {
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
    <form onSubmit={handleSubmit} className="space-y-4">
      <div className="grid grid-cols-2 gap-2 rounded-2xl bg-neutral-950/60 p-1">
        <button
          type="button"
          onClick={() => setSide("buy")}
          className={`rounded-xl py-2.5 text-sm font-semibold transition-all ${
            side === "buy"
              ? "bg-emerald-500 text-emerald-950 shadow-lg shadow-emerald-950/35"
              : "text-neutral-500 hover:bg-neutral-900 hover:text-emerald-300"
          }`}
        >
          Buy
        </button>
        <button
          type="button"
          onClick={() => setSide("sell")}
          className={`rounded-xl py-2.5 text-sm font-semibold transition-all ${
            side === "sell"
              ? "bg-rose-500 text-rose-950 shadow-lg shadow-rose-950/35"
              : "text-neutral-500 hover:bg-neutral-900 hover:text-rose-300"
          }`}
        >
          Sell
        </button>
      </div>

      <div className="grid grid-cols-4 gap-1.5">
        {ORDER_TYPES.map((type) => (
          <button
            key={type.value}
            type="button"
            onClick={() => setOrderType(type.value)}
            className={`min-w-0 rounded-xl border px-1.5 py-2 text-[11px] font-semibold uppercase tracking-normal transition-colors ${
              orderType === type.value
                ? "border-neutral-500 bg-neutral-700/45 text-neutral-100"
                : "border-neutral-800 bg-neutral-900/60 text-neutral-500 hover:border-neutral-700"
            }`}
          >
            {type.label}
          </button>
        ))}
      </div>

      {requiresPrice && (
        <div>
          <label className="mb-1.5 block text-xs font-medium text-neutral-400">Price</label>
          <input
            type="text"
            inputMode="decimal"
            pattern={DECIMAL_INPUT_PATTERN}
            maxLength={10}
            value={price}
            onChange={(e) => setPrice(cleanDecimalInput(e.target.value, MAX_PRICE))}
            placeholder="0.00"
            required
            className="field w-full px-3 py-2.5 text-sm font-mono placeholder:text-neutral-700"
          />
        </div>
      )}

      <div>
        <label className="mb-1.5 block text-xs font-medium text-neutral-400">Quantity</label>
        <input
          type="text"
          inputMode="decimal"
          pattern={DECIMAL_INPUT_PATTERN}
          maxLength={10}
          value={qty}
          onChange={(e) => setQty(cleanDecimalInput(e.target.value, MAX_QTY))}
          placeholder="0.00"
          required
          className="field w-full px-3 py-2.5 text-sm font-mono placeholder:text-neutral-700"
        />
      </div>

      <button
        type="submit"
        disabled={submitting}
        className={`w-full rounded-xl py-3 text-sm font-bold transition-all disabled:opacity-50 ${
          side === "buy"
            ? "bg-emerald-400 text-emerald-950 shadow-lg shadow-emerald-950/40 hover:bg-emerald-300"
            : "bg-rose-400 text-rose-950 shadow-lg shadow-rose-950/40 hover:bg-rose-300"
        }`}
      >
        {submitting
          ? "Placing..."
          : side === "buy"
            ? `Buy ${symbol}`
            : `Sell ${symbol}`}
      </button>

      {status && (
        <div className="rounded-xl border border-neutral-800 bg-neutral-950/60 px-3 py-2 font-mono text-xs leading-relaxed text-neutral-400 break-all">
          {status}
        </div>
      )}
    </form>
  );
}
