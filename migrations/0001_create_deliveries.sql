CREATE TABLE deliveries (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event text NOT NULL,
    attempts integer NOT NULL DEFAULT 0,
    status text NOT NULL DEFAULT 'pending',

    CONSTRAINT deliveries_attempts_nonnegative CHECK (attempts >= 0),
    CONSTRAINT deliveries_status_valid CHECK (status IN ('pending', 'succeeded', 'failed'))
);
