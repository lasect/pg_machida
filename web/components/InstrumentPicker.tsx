"use client";

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
  const { data, error } = useSWR<Instrument[]>("/api/instruments", fetcher, {
    refreshInterval: 10000,
  });

  if (error) return <div className="text-red-400 text-sm">Failed to load</div>;
  if (!data) return <div className="text-neutral-500 text-sm">Loading...</div>;

  return (
    <select
      value={selected ?? ""}
      onChange={(e) => onSelect(e.target.value)}
      className="bg-neutral-800 border border-neutral-700 rounded px-3 py-1.5 text-sm text-white font-mono focus:outline-none focus:border-blue-500"
    >
      <option value="" disabled>
        Select instrument
      </option>
      {data.map((inst) => (
        <option key={inst.id} value={inst.symbol}>
          {inst.symbol}
        </option>
      ))}
    </select>
  );
}
