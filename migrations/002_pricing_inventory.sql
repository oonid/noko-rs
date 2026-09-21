CREATE TABLE variant_prices (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    variant_id uuid NOT NULL REFERENCES product_variants(id),
    currency_code text NOT NULL CHECK (currency_code = 'IDR'),
    amount bigint NOT NULL CHECK (amount >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (variant_id, currency_code)
);

CREATE TABLE inventory_locations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    code text NOT NULL UNIQUE,
    name text NOT NULL,
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO inventory_locations (code, name, active) VALUES ('MAIN', 'Main', true);

CREATE TABLE inventory_items (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    variant_id uuid NOT NULL UNIQUE REFERENCES product_variants(id),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE inventory_levels (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    inventory_item_id uuid NOT NULL REFERENCES inventory_items(id),
    location_id uuid NOT NULL REFERENCES inventory_locations(id),
    stocked_quantity bigint NOT NULL CHECK (stocked_quantity >= 0) DEFAULT 0,
    reserved_quantity bigint NOT NULL CHECK (reserved_quantity >= 0) DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (inventory_item_id, location_id)
);

CREATE TABLE inventory_adjustments (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    inventory_item_id uuid NOT NULL REFERENCES inventory_items(id),
    location_id uuid NOT NULL REFERENCES inventory_locations(id),
    delta bigint NOT NULL CHECK (delta != 0),
    reason text NOT NULL,
    note text,
    actor_id uuid,
    created_at timestamptz NOT NULL DEFAULT now()
);
