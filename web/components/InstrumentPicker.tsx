"use client";

import { useEffect, useRef, useState } from "react";
import useSWR from "swr";
import { fetcher } from "@/lib/fetcher";

type Instrument = {
  id: number;
  symbol: string;
  status: string;
};

export default function InstrumentPicker({
  selected,
  onSelect,
}: {
  selected: string | null;
  onSelect: (symbol: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const pickerRef = useRef<HTMLDivElement>(null);
  const { data, error } = useSWR<Instrument[]>("/api/instruments", fetcher, {
    refreshInterval: 10000,
  });

  useEffect(() => {
    if (!open) return;

    function handlePointerDown(event: PointerEvent) {
      if (!pickerRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [open]);

  if (error) return <div className="text-sm text-red-300">Failed to load</div>;
  if (!data) return <div className="text-sm text-neutral-500">Loading...</div>;

  return (
    <div ref={pickerRef} className="relative w-full font-mono text-sm">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="field flex w-full items-center justify-between gap-3 px-3 py-2 text-left"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label="Instrument"
      >
        <span className={selected ? "text-white" : "text-neutral-500"}>
          {selected ?? "Select instrument"}
        </span>
        <span className="text-xs text-neutral-500">{open ? "Close" : "Market"}</span>
      </button>

      {open && (
        <div
          role="listbox"
          className="absolute left-0 right-0 top-[calc(100%+0.5rem)] z-30 overflow-hidden rounded-xl border border-neutral-700 bg-neutral-950 shadow-2xl shadow-black/40"
        >
          {data.map((inst) => {
            const active = inst.symbol === selected;

            return (
              <button
                key={inst.id}
                type="button"
                role="option"
                aria-selected={active}
                onClick={() => {
                  onSelect(inst.symbol);
                  setOpen(false);
                }}
                className={`flex w-full items-center justify-between px-3 py-2 text-left transition hover:bg-neutral-800/80 ${
                  active ? "bg-neutral-700/45 text-neutral-100" : "text-neutral-200"
                }`}
              >
                <span>{inst.symbol}</span>
                <span className="text-[0.65rem] uppercase tracking-[0.18em] text-neutral-500">
                  {active ? "Live" : inst.status}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
