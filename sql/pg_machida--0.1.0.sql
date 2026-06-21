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
    buy_order_id        UUID        NOT NULL REFERENCES clob.orders(id),
    sell_order_id       UUID        NOT NULL REFERENCES clob.orders(id),
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
