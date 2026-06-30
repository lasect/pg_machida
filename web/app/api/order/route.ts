import { NextRequest, NextResponse } from "next/server";
import { placeOrder, cancelOrder } from "@/lib/queries";

export async function POST(req: NextRequest) {
  try {
    const body = await req.json();
    const { instrument, side, orderType, qty, participant, price, stpMode } =
      body;

    if (!instrument || !side || !orderType || !participant || qty == null) {
      return NextResponse.json(
        { error: "Missing required fields" },
        { status: 400 }
      );
    }

    if (qty <= 0) {
      return NextResponse.json(
        { error: "Quantity must be positive" },
        { status: 400 }
      );
    }

    if (orderType === "limit" && (price == null || price <= 0)) {
      return NextResponse.json(
        { error: "Limit orders require a positive price" },
        { status: 400 }
      );
    }

    const result = await placeOrder({
      instrument,
      side,
      orderType,
      qty,
      participant,
      price: price ?? undefined,
      stpMode: stpMode ?? "cancel_newest",
    });

    if (!result) {
      return NextResponse.json(
        { error: "Order placement failed" },
        { status: 500 }
      );
    }

    return NextResponse.json(result);
  } catch (e: any) {
    return NextResponse.json({ error: e.message }, { status: 500 });
  }
}

export async function DELETE(req: NextRequest) {
  const id = req.nextUrl.searchParams.get("id");

  if (!id) {
    return NextResponse.json({ error: "id required" }, { status: 400 });
  }

  try {
    const result = await cancelOrder(id);
    return NextResponse.json({ cancelled: result?.cancel_order ?? false });
  } catch (e: any) {
    return NextResponse.json({ error: e.message }, { status: 500 });
  }
}
