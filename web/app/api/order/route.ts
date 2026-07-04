import { NextRequest, NextResponse } from "next/server";
import { placeOrder, cancelOrder } from "@/lib/queries";

const MAX_PARTICIPANT_LENGTH = 32;
const MAX_PRICE = 1_000_000;
const MAX_QTY = 1_000_000;
const PARTICIPANT_PATTERN = /^[a-zA-Z0-9_-]+$/;

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "Unexpected error";
}

function hasValidDecimalPlaces(value: number) {
  const text = value.toString();
  const decimals = text.split(".")[1] ?? "";

  return !text.includes("e") && decimals.length <= 2;
}

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

    if (
      typeof participant !== "string" ||
      participant.length > MAX_PARTICIPANT_LENGTH ||
      !PARTICIPANT_PATTERN.test(participant)
    ) {
      return NextResponse.json(
        { error: "Trader ID must be 1-32 letters, numbers, underscores, or dashes" },
        { status: 400 }
      );
    }

    if (!Number.isFinite(qty) || qty <= 0 || qty > MAX_QTY || !hasValidDecimalPlaces(qty)) {
      return NextResponse.json(
        { error: `Quantity must be greater than 0, no more than ${MAX_QTY}, and use up to 2 decimals` },
        { status: 400 }
      );
    }

    if (
      orderType === "limit" &&
      (price == null ||
        !Number.isFinite(price) ||
        price <= 0 ||
        price > MAX_PRICE ||
        !hasValidDecimalPlaces(price))
    ) {
      return NextResponse.json(
        { error: `Limit orders require a price greater than 0, no more than ${MAX_PRICE}, and up to 2 decimals` },
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
  } catch (e: unknown) {
    return NextResponse.json({ error: getErrorMessage(e) }, { status: 500 });
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
  } catch (e: unknown) {
    return NextResponse.json({ error: getErrorMessage(e) }, { status: 500 });
  }
}
