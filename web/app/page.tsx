"use client";

import { useState, useCallback } from "react";
import InstrumentPicker from "@/components/InstrumentPicker";
import OrderForm from "@/components/OrderForm";
import OrderBook from "@/components/OrderBook";
import TradeTape from "@/components/TradeTape";
import OpenOrders from "@/components/OpenOrders";

const MAX_PARTICIPANT_LENGTH = 32;

function cleanParticipant(value: string) {
  return value.replace(/[^a-zA-Z0-9_-]/g, "").slice(0, MAX_PARTICIPANT_LENGTH);
}

export default function Home() {
  const [symbol, setSymbol] = useState<string>("");
  const [participant, setParticipant] = useState<string>("trader_1");
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((t) => t + 1), []);

  return (
    <div className="flex min-h-screen flex-col">
      <header className="sticky top-0 z-20 shrink-0 border-b border-neutral-800/80 bg-neutral-950/82 px-4 py-3 shadow-2xl shadow-black/20 backdrop-blur-xl sm:px-6">
        <div className="mx-auto flex max-w-7xl flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div className="flex items-center gap-3">
            <div>
              <h1 className="font-mono text-xl font-bold tracking-tight text-white">
                pg_machida
              </h1>
            </div>
          </div>
          <div className="grid gap-2 sm:grid-cols-[minmax(0,14rem)_minmax(0,10rem)] lg:flex lg:items-center lg:gap-3">
            <InstrumentPicker selected={symbol || null} onSelect={setSymbol} />
            <input
              type="text"
              value={participant}
              onChange={(e) => setParticipant(cleanParticipant(e.target.value))}
              placeholder="Trader ID"
              maxLength={MAX_PARTICIPANT_LENGTH}
              pattern="[a-zA-Z0-9_-]{1,32}"
              title="Use 1-32 letters, numbers, underscores, or dashes"
              className="field w-full px-3 py-2 text-sm font-mono placeholder:text-neutral-600 lg:w-40"
              aria-label="Trader ID"
            />
          </div>
        </div>
      </header>

      <main className="mx-auto grid w-full max-w-7xl flex-1 grid-cols-1 gap-4 p-2 sm:p-4 lg:grid-cols-[minmax(20rem,1fr)_minmax(22rem,1.35fr)_minmax(18rem,0.95fr)] lg:items-stretch">
        <div className="flex min-h-0 flex-col gap-4">
          <section className="panel p-4 sm:p-5">
            <div className="mb-4 flex items-center justify-between">
              <h2 className="panel-heading">Place Order</h2>
              {symbol && (
                <span className="rounded-full border border-neutral-700 bg-neutral-900/80 px-2.5 py-1 font-mono text-xs text-neutral-300">
                  {symbol}
                </span>
              )}
            </div>
            {symbol ? (
              <OrderForm
                symbol={symbol}
                participant={participant}
                onOrderPlaced={refresh}
              />
            ) : (
              <div className="empty-state px-4 text-sm">
                Select an instrument above to unlock the order ticket.
              </div>
            )}
          </section>

          <section className="panel flex min-h-[16rem] flex-1 flex-col overflow-hidden p-4 sm:p-5">
            <div className="mb-4 flex items-center justify-between">
              <h2 className="panel-heading">Open Orders</h2>
              <span className="font-mono text-xs text-neutral-600">{participant || "no trader"}</span>
            </div>
            <OpenOrders
              participant={participant}
              symbol={symbol}
              onMutate={refresh}
            />
          </section>
        </div>

        <section className="panel flex min-h-[28rem] flex-col p-4 sm:p-5">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="panel-heading">Order Book</h2>
            <span className="rounded-full bg-neutral-900 px-2.5 py-1 font-mono text-xs text-neutral-500">2s refresh</span>
          </div>
          <div className="min-h-0 flex-1 overflow-auto">
            <OrderBook key={`${symbol}-${tick}`} symbol={symbol} />
          </div>
        </section>

        <section className="panel flex min-h-[24rem] flex-col p-4 sm:p-5 md:col-span-2 lg:col-span-1">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="panel-heading">Trade Tape</h2>
            {symbol && <span className="font-mono text-xs text-neutral-500">latest 30</span>}
          </div>
          <div className="min-h-0 flex-1 overflow-auto">
            <TradeTape key={`${symbol}-${tick}`} symbol={symbol} />
          </div>
        </section>
      </main>
    </div>
  );
}
