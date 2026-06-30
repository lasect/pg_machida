import { NextRequest, NextResponse } from "next/server";
import { getBook } from "@/lib/queries";

export async function GET(req: NextRequest) {
  const symbol = req.nextUrl.searchParams.get("symbol");
  const depth = parseInt(req.nextUrl.searchParams.get("depth") ?? "10");

  if (!symbol) {
    return NextResponse.json({ error: "symbol required" }, { status: 400 });
  }

  try {
    const result = await getBook(symbol, depth);
    return NextResponse.json([...result]);
  } catch (e: any) {
    return NextResponse.json(
      { error: e.message ?? "Failed to fetch book" },
      { status: 500 }
    );
  }
}
