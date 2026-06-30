"use client";

import { useState, useCallback } from "react";
import InstrumentPicker from "@/components/InstrumentPicker";
import OrderForm from "@/components/OrderForm";
import OrderBook from "@/components/OrderBook";
import TradeTape from "@/components/TradeTape";
import OpenOrders from "@/components/OpenOrders";

export default function Home() {
  const [symbol, setSymbol] = useState<string>("");
  const [participant, setParticipant] = useState<string>("trader_1");
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  return (
    <div className="flex flex-col h-screen">
      {/* Header */}
      <header className="flex items-center justify-between px-4 py-2 border-b border-neutral-800 bg-neutral-900 shrink-0">
        <div className="flex items-center gap-3">
          <h1 className="text-lg font-bold font-mono tracking-tight">
            pg_machida
          </h1>
          <span className="text-neutral-600 text-xs font-mono">CLOB Exchange</span>
        </div>
        <div className="flex items-center gap-3">
          <InstrumentPicker selected={symbol || null} onSelect={setSymbol} />
          <input
            type="text"
            value={participant}
            onChange={(e) => setParticipant(e.target.value)}
            placeholder="Trader ID"
            className="bg-neutral-800 border border-neutral-700 rounded px-3 py-1.5 text-sm text-white font-mono placeholder:text-neutral-600 focus:outline-none focus:border-blue-500 w-36"
          />
        </div>
      </header>

      {/* Main grid */}
      <main className="flex-1 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 p-4 min-h-0">
        {/* Left column — Order Form + Open Orders */}
        <div className="flex flex-col gap-4 min-h-0">
          <section className="bg-neutral-900 border border-neutral-800 rounded-lg p-4">
            <h2 className="text-xs font-semibold text-neutral-400 uppercase tracking-wide mb-3">
              Place Order
            </h2>
            {symbol ? (
              <OrderForm
                symbol={symbol}
                participant={participant}
                onOrderPlaced={refresh}
              />
            ) : (
              <div className="text-neutral-600 text-sm">
                Select an instrument to trade
              </div>
            )}
          </section>

          <section className="bg-neutral-900 border border-neutral-800 rounded-lg p-4 flex-1 overflow-hidden">
            <h2 className="text-xs font-semibold text-neutral-400 uppercase tracking-wide mb-3">
              Open Orders
            </h2>
            <OpenOrders
              participant={participant}
              symbol={symbol}
              onMutate={refresh}
            />
          </section>
        </div>

        {/* Middle column — Order Book */}
        <section className="bg-neutral-900 border border-neutral-800 rounded-lg p-4 flex flex-col min-h-0">
          <h2 className="text-xs font-semibold text-neutral-400 uppercase tracking-wide mb-3">
            Order Book
          </h2>
          <div className="flex-1 overflow-auto">
            <OrderBook key={`${symbol}-${tick}`} symbol={symbol} />
          </div>
        </section>

        {/* Right column — Trade Tape */}
        <section className="bg-neutral-900 border border-neutral-800 rounded-lg p-4 flex flex-col min-h-0">
          <h2 className="text-xs font-semibold text-neutral-400 uppercase tracking-wide mb-3">
            Trade Tape
          </h2>
          <div className="flex-1 overflow-auto">
            <TradeTape key={`${symbol}-${tick}`} symbol={symbol} />
          </div>
        </section>
      </main>
    </div>
  );
}
