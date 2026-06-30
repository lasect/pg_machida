import { NextResponse } from "next/server";
import { getInstruments } from "@/lib/queries";

export async function GET() {
  try {
    const rows = await getInstruments();
    return NextResponse.json(rows);
  } catch (e) {
    return NextResponse.json(
      { error: "Failed to fetch instruments" },
      { status: 500 }
    );
  }
}
