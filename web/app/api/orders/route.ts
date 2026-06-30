import { NextRequest, NextResponse } from "next/server";
import { getOpenOrders } from "@/lib/queries";

export async function GET(req: NextRequest) {
  const participant = req.nextUrl.searchParams.get("participant");
  const instrument = req.nextUrl.searchParams.get("instrument") ?? null;

  if (!participant) {
    return NextResponse.json(
      { error: "participant required" },
      { status: 400 }
    );
  }

  try {
    const result = await getOpenOrders(participant, instrument);
    return NextResponse.json([...result]);
  } catch (e) {
    return NextResponse.json(
      { error: "Failed to fetch open orders" },
      { status: 500 }
    );
  }
}
