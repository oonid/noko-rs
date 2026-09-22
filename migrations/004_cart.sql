CREATE TABLE carts (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id uuid NOT NULL REFERENCES customers(id),
    currency_code text NOT NULL DEFAULT 'IDR' CHECK (currency_code = 'IDR'),
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'completed')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz
);

CREATE UNIQUE INDEX carts_one_active_per_customer
    ON carts(customer_id)
    WHERE status = 'active';

CREATE TABLE cart_items (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    cart_id uuid NOT NULL REFERENCES carts(id) ON DELETE CASCADE,
    variant_id uuid NOT NULL REFERENCES product_variants(id),
    variant_title text NOT NULL,
    sku text NOT NULL,
    quantity bigint NOT NULL CHECK (quantity > 0),
    unit_price bigint NOT NULL CHECK (unit_price >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (cart_id, variant_id)
);

CREATE TABLE cart_addresses (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    cart_id uuid NOT NULL REFERENCES carts(id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('shipping', 'billing')),
    recipient_name text NOT NULL,
    phone text,
    address_line_1 text NOT NULL,
    address_line_2 text,
    city text NOT NULL,
    province text NOT NULL,
    postal_code text NOT NULL,
    country_code text NOT NULL,
    UNIQUE (cart_id, kind)
);
