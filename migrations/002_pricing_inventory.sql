CREATE TABLE variant_prices (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    variant_id uuid NOT NULL REFERENCES product_variants(id),
    currency_code text NOT NULL CHECK (currency_code = 'IDR'),
    amount bigint NOT NULL CHECK (amount >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (variant_id, currency_code)
);
