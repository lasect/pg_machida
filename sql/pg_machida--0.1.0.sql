CREATE SCHEMA clob;

CREATE TABLE clob.instruments (
    id            BIGSERIAL PRIMARY KEY,
    symbol        TEXT        NOT NULL UNIQUE,
    tick_size     NUMERIC     NOT NULL DEFAULT 0.01,
    lot_size      NUMERIC     NOT NULL DEFAULT 1,
    max_ticks     INT         NOT NULL DEFAULT 10000000,
    status        TEXT        NOT NULL DEFAULT 'active',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE clob.participants (
    id            TEXT        PRIMARY KEY,
    display_name  TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE clob.orders (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    instrument_id  BIGINT      NOT NULL REFERENCES clob.instruments(id),
    participant_id TEXT        NOT NULL REFERENCES clob.participants(id),
    side           TEXT        NOT NULL CHECK (side IN ('buy', 'sell')),
    order_type     TEXT        NOT NULL CHECK (order_type IN ('limit', 'market', 'ioc', 'fok')),
    price          NUMERIC,
    qty            NUMERIC     NOT NULL,
    remaining      NUMERIC     NOT NULL,
    status         TEXT        NOT NULL DEFAULT 'open',
    stp_mode       TEXT        NOT NULL DEFAULT 'cancel_newest',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ON clob.orders (instrument_id, price DESC, created_at ASC)
    WHERE side = 'buy' AND status IN ('open', 'partially_filled');
CREATE INDEX ON clob.orders (instrument_id, price ASC, created_at ASC)
    WHERE side = 'sell' AND status IN ('open', 'partially_filled');
CREATE INDEX ON clob.orders (participant_id);

CREATE TABLE clob.trades (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    instrument_id       BIGINT      NOT NULL REFERENCES clob.instruments(id),
    buy_order_id        UUID        NOT NULL,
    sell_order_id       UUID        NOT NULL,
    buy_participant_id  TEXT        NOT NULL,
    sell_participant_id TEXT        NOT NULL,
    price               NUMERIC     NOT NULL,
    qty                 NUMERIC     NOT NULL,
    executed_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ON clob.trades (instrument_id, executed_at DESC);
CREATE INDEX ON clob.trades (buy_participant_id);
CREATE INDEX ON clob.trades (sell_participant_id);

CREATE TABLE clob.book_snapshots (
    id            BIGSERIAL   PRIMARY KEY,
    instrument_id BIGINT      NOT NULL REFERENCES clob.instruments(id),
    side          TEXT        NOT NULL,
    price         NUMERIC     NOT NULL,
    qty           NUMERIC     NOT NULL,
    order_count   INT         NOT NULL,
    snapshot_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- SQL wrapper: create_instrument
CREATE OR REPLACE FUNCTION clob.create_instrument(
    symbol    TEXT,
    tick_size NUMERIC DEFAULT 0.01,
    lot_size  NUMERIC DEFAULT 1,
    max_ticks INT     DEFAULT 10000000
) RETURNS BIGINT LANGUAGE sql AS $$
    SELECT clob_create_instrument(
        symbol,
        tick_size::float8,
        lot_size::float8,
        max_ticks
    );
$$;

-- SQL wrapper: create_participant
CREATE OR REPLACE FUNCTION clob.create_participant(
    participant_id TEXT,
    display_name   TEXT DEFAULT NULL
) RETURNS VOID LANGUAGE sql AS $$
    INSERT INTO clob.participants (id, display_name)
    VALUES (participant_id, display_name)
    ON CONFLICT (id) DO UPDATE SET display_name = EXCLUDED.display_name;
$$;

-- SQL wrapper: place_order
CREATE OR REPLACE FUNCTION clob.place_order(
    instrument  TEXT,
    side        TEXT,
    order_type  TEXT,
    participant TEXT,
    qty         NUMERIC DEFAULT 0,
    price       NUMERIC DEFAULT NULL,
    stp_mode    TEXT    DEFAULT 'cancel_newest'
) RETURNS TABLE (
    order_id    TEXT,
    status      TEXT,
    filled_qty  FLOAT8,
    avg_price   FLOAT8
) LANGUAGE sql AS $$
    SELECT * FROM clob_place_order(
        instrument,
        side,
        order_type,
        qty::float8,
        participant,
        price::float8,
        stp_mode
    );
$$;

-- SQL wrapper: cancel_order
CREATE OR REPLACE FUNCTION clob.cancel_order(
    order_id UUID
) RETURNS BOOLEAN LANGUAGE sql AS $$
    SELECT clob_cancel_order(order_id::text);
$$;

-- SQL wrapper: get_book
CREATE OR REPLACE FUNCTION clob.get_book(
    instrument TEXT,
    depth      INT DEFAULT 10
) RETURNS TABLE (
    side         TEXT,
    price        FLOAT8,
    qty          FLOAT8,
    order_count  INT
) LANGUAGE sql AS $$
    SELECT * FROM clob_get_book(instrument, depth);
$$;

-- SQL wrapper: get_trades (pure DB query, no engine call)
CREATE OR REPLACE FUNCTION clob.get_trades(
    instrument TEXT,
    lim        INT DEFAULT 50
) RETURNS TABLE (
    id                  UUID,
    price               NUMERIC,
    qty                 NUMERIC,
    buy_participant_id  TEXT,
    sell_participant_id TEXT,
    executed_at         TIMESTAMPTZ
) LANGUAGE sql AS $$
    SELECT t.id, t.price, t.qty, t.buy_participant_id, t.sell_participant_id, t.executed_at
    FROM clob.trades t
    JOIN clob.instruments i ON i.id = t.instrument_id
    WHERE i.symbol = instrument
    ORDER BY t.executed_at DESC
    LIMIT lim;
$$;

-- SQL wrapper: get_open_orders
CREATE OR REPLACE FUNCTION clob.get_open_orders(
    participant TEXT,
    instrument  TEXT DEFAULT NULL
) RETURNS TABLE (
    order_id       TEXT,
    instrument_id  BIGINT,
    side           TEXT,
    order_type     TEXT,
    price          FLOAT8,
    qty            FLOAT8,
    remaining      FLOAT8,
    status         TEXT
) LANGUAGE sql AS $$
    SELECT * FROM clob_get_open_orders(participant, instrument);
$$;

-- SQL wrapper: mass_cancel
CREATE OR REPLACE FUNCTION clob.mass_cancel(
    participant TEXT,
    instrument  TEXT
) RETURNS INT LANGUAGE sql AS $$
    SELECT clob_mass_cancel(participant, instrument);
$$;

-- SQL wrapper: halt_instrument
CREATE OR REPLACE FUNCTION clob.halt_instrument(
    instrument TEXT
) RETURNS VOID LANGUAGE sql AS $$
    SELECT clob_halt_instrument(instrument);
$$;

-- SQL wrapper: resume_instrument
CREATE OR REPLACE FUNCTION clob.resume_instrument(
    instrument TEXT
) RETURNS VOID LANGUAGE sql AS $$
    SELECT clob_resume_instrument(instrument);
$$;

-- SQL wrapper: snapshot_book
CREATE OR REPLACE FUNCTION clob.snapshot_book(
    instrument TEXT
) RETURNS VOID LANGUAGE sql AS $$
    SELECT clob_snapshot_book(instrument);
$$;
