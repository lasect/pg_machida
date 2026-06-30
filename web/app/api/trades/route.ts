import { NextRequest, NextResponse } from "next/server";
import { getTrades } from "@/lib/queries";

export async function GET(req: NextRequest) {
  const symbol = req.nextUrl.searchParams.get("symbol");
  const limit = parseInt(req.nextUrl.searchParams.get("limit") ?? "50");

  if (!symbol) {
    return NextResponse.json({ error: "symbol required" }, { status: 400 });
  }

  try {
    const result = await getTrades(symbol, limit);
    return NextResponse.json([...result]);
  } catch (e) {
    return NextResponse.json(
      { error: "Failed to fetch trades" },
      { status: 500 }
    );
  }
}
