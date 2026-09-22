CREATE TABLE actors (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    kind text NOT NULL CHECK (kind IN ('human', 'agent', 'service')),
    auth_subject text NOT NULL UNIQUE,
    display_name text NOT NULL,
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE inventory_adjustments
    ADD CONSTRAINT inventory_adjustments_actor_fk
    FOREIGN KEY (actor_id) REFERENCES actors(id);

CREATE TABLE customers (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id uuid NOT NULL UNIQUE REFERENCES actors(id),
    email text NOT NULL UNIQUE,
    phone text,
    first_name text NOT NULL,
    last_name text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE customer_addresses (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id uuid NOT NULL REFERENCES customers(id),
    label text NOT NULL,
    recipient_name text NOT NULL,
    phone text,
    address_line_1 text NOT NULL,
    address_line_2 text,
    city text NOT NULL,
    province text NOT NULL,
    postal_code text NOT NULL,
    country_code text NOT NULL,
    is_default boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
