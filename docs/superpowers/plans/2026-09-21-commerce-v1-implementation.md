# Commerce V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Build the first usable noko-rs commerce milestone from Catalog through atomic Order placement and cancellation, preserving the approved modular-monolith boundaries and PostgreSQL-native correctness.

**Architecture:** One Rust binary and one crate use Axum + SQLx against one PostgreSQL database. Domain modules own their mutations; cross-module behavior lives in application operations and may span one PostgreSQL transaction. NocoDB remains an out-of-band operator UI with a restricted PostgreSQL role and typed Rust /ops commands for business transitions.

**Tech Stack:** Rust 2024 edition; Axum 0.8.9; Tokio 1.52.x; SQLx 0.9.0 with PostgreSQL/migrate/uuid/time support; PostgreSQL 15+; Serde 1.0; thiserror 2.0; tracing + tracing-subscriber 0.3; tower-http 0.6.11; UUID 1.26.x; real PostgreSQL integration tests via sqlx::test.

**Spec:** docs/superpowers/specs/2026-09-20-commerce-modular-monolith-design.md

## Global Constraints

- One Rust binary, one Rust crate initially, one PostgreSQL database, one NocoDB deployment.
- PostgreSQL is the source of commerce truth; Rust owns schema evolution and business transitions.
- Commerce modules are peers. A module never mutates another module's state directly.
- Cross-module writes belong in application operations; one PostgreSQL transaction may span modules.
- Registered customers only.
- Exactly one active/default inventory location in V1: code MAIN.
- IDR only; one fixed IDR price per Variant.
- Free shipping; total equals subtotal; no tax.
- One active cart per Customer.
- Adding to Cart does not reserve inventory.
- Checkout reserves inventory; cancellation releases reservation.
- stocked_quantity >= 0 and reserved_quantity >= 0 are hard constraints.
- stocked_quantity may be less than reserved_quantity after truthful physical adjustment; available_quantity may therefore be negative.
- New reservations require available_quantity >= requested_quantity.
- CartItem snapshots variant_title, sku, and unit_price; OrderItem copies the CartItem commercial snapshot.
- CustomerAddress is copied into CartAddress, then CartAddress is copied into OrderAddress.
- complete_cart must reject stale price with 409 CART_PRICE_CHANGED; it must not silently reprice.
- orders.cart_id is UNIQUE and checkout is idempotent.
- Critical inventory rows are locked with SELECT ... FOR UPDATE in deterministic ID order.
- AI agents never write PostgreSQL directly.
- NocoDB may directly edit only specifically granted maintained-data columns; business commands go through Rust.
- No Payment, Fulfillment implementation, POINT, promotions, tax, paid shipping, multi-location allocator, Redis, Event Bus, Worker, outbox, saga, or generic Workflow Engine in this milestone.
- All transaction, locking, constraint, and concurrency behavior is tested against real PostgreSQL.

## Implementation choices fixed by this plan

These were intentionally deferred by the architecture and are now fixed for V1 implementation:

- IDs: PostgreSQL UUID columns with DEFAULT gen_random_uuid(); Rust uses uuid::Uuid.
- Time: PostgreSQL TIMESTAMPTZ; Rust uses time::OffsetDateTime.
- Money and quantities: signed BIGINT in PostgreSQL / i64 in Rust, with non-negative or positive CHECK constraints as appropriate.
- SQL access: SQLx runtime queries + FromRow; do not add a generic repository trait/ORM abstraction.
- Test DB isolation: sqlx::test with automatic migrations and a PostgreSQL URL whose test user can create temporary test databases.
- Store authentication during V1 development: an explicit dev/test header adapter, disabled unless AUTH_MODE=dev_header.
- Ops authentication: static Bearer token mapped to a configured service Actor; human NocoDB identity is provenance only.
- Transient transaction retries: at most 2 retries after the first attempt; only SQLSTATE 40P01 (deadlock) and 40001 (serialization failure) are retryable.

## Review Focus

1. **Race between checkout and direct NocoDB price/Variant update:** checkout must lock current Variant/Price rows while validating so accepted checkout cannot race with an administrative change.
2. **Two concurrent carts competing for one unit:** exactly one order succeeds; the loser receives INSUFFICIENT_INVENTORY and no partial order/reservation rows.
3. **Physical adjustment below outstanding reservations:** adjustment succeeds if stocked remains >= 0; availability may become negative; later checkout is rejected.
4. **Retry after an already-committed checkout:** retry returns the existing Order via the locked Cart + UNIQUE orders.cart_id path and does not reserve twice.
5. **Cross-customer access:** authenticated Customer A cannot mutate or complete Customer B's cart, even with a valid cart UUID.

---

## File map

Create this shape and keep files focused:

~~~text
Cargo.toml
Cargo.lock
rust-toolchain.toml
src/
  lib.rs
  main.rs
  app.rs
  config.rs
  db.rs
  error.rs
  auth.rs
  api/
    mod.rs
    store/
      mod.rs
      products.rs
      me.rs
      carts.rs
      orders.rs
    ops/
      mod.rs
      catalog.rs
      inventory.rs
      orders.rs
  application/
    mod.rs
    create_sellable_variant.rs
    create_or_get_active_cart.rs
    add_cart_item.rs
    set_cart_address.rs
    adjust_inventory.rs
    complete_cart.rs
    cancel_order.rs
    retry.rs
  catalog/
    mod.rs
    model.rs
    repository.rs
  pricing/
    mod.rs
    model.rs
    repository.rs
  inventory/
    mod.rs
    model.rs
    repository.rs
    service.rs
  actor/
    mod.rs
    model.rs
    repository.rs
  customer/
    mod.rs
    model.rs
    repository.rs
  cart/
    mod.rs
    model.rs
    repository.rs
  order/
    mod.rs
    model.rs
    repository.rs
migrations/
  001_catalog.sql
  002_pricing_inventory.sql
  003_actor_customer.sql
  004_cart.sql
  005_order_reservation.sql
tests/
  foundation.rs
  catalog.rs
  inventory.rs
  customer_auth.rs
  operations.rs
  cart.rs
  checkout.rs
  cancellation.rs
  nocodb_role.rs
ops/
  sql/
    nocodb_grants.sql
  README.md
docs/
  architecture.md
  modules/
    catalog.md
    pricing.md
    inventory.md
    actor-customer.md
    cart.md
    order.md
.github/workflows/ci.yml
~~~

---

### Task 1: Foundation, test harness, and approved-spec cleanup

**Files:**
- Create: Cargo.toml
- Create: rust-toolchain.toml
- Create: src/lib.rs
- Create: src/main.rs
- Create: src/app.rs
- Create: src/config.rs
- Create: src/db.rs
- Create: src/error.rs
- Create: tests/foundation.rs
- Create: .github/workflows/ci.yml
- Modify: docs/superpowers/specs/2026-09-20-commerce-modular-monolith-design.md

**Interfaces:**
- Produces: AppState { pool: PgPool, config: Arc<Config> }
- Produces: Config::from_env() -> Result<Config, ConfigError>
- Produces: build_router(AppState) -> axum::Router
- Produces: AppError -> IntoResponse
- Produces: GET /health/live and GET /health/ready

- [ ] **Step 1: Correct the two stale illustrative lines in the approved spec**

Change the status to Approved architecture and correct the stale Section 32 example so it says physical adjustment enforces stocked_quantity >= 0 and may produce negative availability. Also update the migration example from 002_inventory.sql to 002_pricing_inventory.sql.

- [ ] **Step 2: Create the Cargo manifest**

Use:

~~~toml
[package]
name = "noko-rs"
version = "0.1.0"
edition = "2024"

[dependencies]
axum = "0.8.9"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sqlx = { version = "0.9.0", features = ["runtime-tokio", "tls-rustls-ring-native-roots", "postgres", "derive", "macros", "migrate", "uuid", "time", "json"] }
thiserror = "2.0.20"
time = { version = "0.3", features = ["serde"] }
tokio = { version = "1.52", features = ["macros", "rt-multi-thread", "signal", "net"] }
tower-http = { version = "0.6.11", features = ["request-id", "trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3.23", features = ["env-filter", "json"] }
uuid = { version = "1.26", features = ["serde", "v4"] }

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
~~~

- [ ] **Step 3: Write the failing health/config test**

~~~rust
#[tokio::test]
async fn config_requires_database_url() {
    unsafe { std::env::remove_var("DATABASE_URL") };
    assert!(noko_rs::config::Config::from_env().is_err());
}
~~~

Also write an HTTP test asserting /health/live returns 200 and /health/ready returns 503 when the pool cannot reach PostgreSQL.

- [ ] **Step 4: Run the focused tests and confirm failure**

Run:

~~~bash
cargo test --test foundation
~~~

Expected: compile/test failure because Config/AppState/router do not exist.

- [ ] **Step 5: Implement minimal configuration, state, errors, and health routes**

Use a Config shape like:

~~~rust
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub auth_mode: String,
    pub nocodb_service_token: Option<String>,
    pub nocodb_service_actor_id: Option<Uuid>,
    pub db_tx_max_retries: u32,
}
~~~

AppState:

~~~rust
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
}
~~~

Readiness must execute SELECT 1 against PostgreSQL; NocoDB availability is not part of readiness.

- [ ] **Step 6: Add request IDs and structured tracing**

Use tower-http SetRequestIdLayer + PropagateRequestIdLayer + TraceLayer. Record request_id, method, URI, status, and latency; never log Authorization.

- [ ] **Step 7: Add CI with PostgreSQL 15**

The workflow must run:

~~~bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
~~~

with DATABASE_URL pointing at the PostgreSQL service.

- [ ] **Step 8: Run verification**

~~~bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --test foundation
~~~

Expected: PASS.

- [ ] **Step 9: Commit**

~~~bash
git add Cargo.toml Cargo.lock rust-toolchain.toml src tests/foundation.rs .github/workflows/ci.yml docs/superpowers/specs/2026-09-20-commerce-modular-monolith-design.md
git commit -m "feat: establish rust service foundation"
~~~

---

### Task 2: Catalog and Pricing schema + Store reads

**Files:**
- Create: migrations/001_catalog.sql
- Create: migrations/002_pricing_inventory.sql
- Create: src/catalog/mod.rs
- Create: src/catalog/model.rs
- Create: src/catalog/repository.rs
- Create: src/pricing/mod.rs
- Create: src/pricing/model.rs
- Create: src/pricing/repository.rs
- Create: src/api/store/products.rs
- Modify: src/api/store/mod.rs
- Modify: src/api/mod.rs
- Test: tests/catalog.rs

**Interfaces:**
- Produces: catalog::repository::get_active_variant(&mut PgConnection, Uuid) -> Result<ProductVariant, AppError>
- Produces: pricing::repository::get_idr_price(&mut PgConnection, Uuid) -> Result<VariantPrice, AppError>
- Produces: GET /store/products and GET /store/products/:id

- [ ] **Step 1: Write failing migration/constraint tests**

Use sqlx::test:

~~~rust
#[sqlx::test(migrations = "./migrations")]
async fn sku_is_unique(pool: PgPool) -> sqlx::Result<()> {
    // insert one product, then two variants with the same SKU;
    // assert the second insert returns a unique-violation database error.
    Ok(())
}
~~~

Also test that variant_prices rejects currency other than IDR and negative amount.

- [ ] **Step 2: Run the test and confirm failure**

~~~bash
cargo test --test catalog
~~~

Expected: FAIL because migrations/models are absent.

- [ ] **Step 3: Implement 001_catalog.sql**

Use the exact core shape:

~~~sql
CREATE TABLE products (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    title text NOT NULL,
    description text NOT NULL DEFAULT '',
    status text NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'active', 'archived')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE product_variants (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id uuid NOT NULL REFERENCES products(id),
    sku text NOT NULL UNIQUE,
    title text NOT NULL,
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
~~~

- [ ] **Step 4: Add Pricing portion of 002_pricing_inventory.sql**

~~~sql
CREATE TABLE variant_prices (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    variant_id uuid NOT NULL REFERENCES product_variants(id),
    currency_code text NOT NULL CHECK (currency_code = 'IDR'),
    amount bigint NOT NULL CHECK (amount >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (variant_id, currency_code)
);
~~~

Task 3 extends this same migration with Inventory tables before it is committed as final migration content.

- [ ] **Step 5: Implement models and read repositories**

Use SQLx FromRow types. Keep ProductVariant and VariantPrice in their owning modules; do not create a shared commerce model module.

- [ ] **Step 6: Implement Store product reads**

GET /store/products returns only active products with active variants and an IDR price. GET /store/products/:id returns 404 for missing/inactive product.

Use one cross-module read query if it keeps the endpoint simple; the read/write asymmetry explicitly allows this.

- [ ] **Step 7: Add HTTP tests**

Assert:
- inactive products are not listed,
- variant price is represented as integer IDR,
- unknown product returns 404,
- no mutation endpoint is exposed under /store/products.

- [ ] **Step 8: Run verification**

~~~bash
cargo test --test catalog
cargo test --all
~~~

Expected: PASS after Task 3 finishes the shared migration; until then run the Catalog tests against a temporary Inventory-free version or complete Steps 1-4 of Task 3 before final verification.

- [ ] **Step 9: Commit together with Task 3 migration completion if necessary**

Do not commit a migration file that will be rewritten after it has been applied outside development.

---

### Task 3: Inventory core, MAIN location, availability, and truthful adjustment

**Files:**
- Complete: migrations/002_pricing_inventory.sql
- Create: src/inventory/mod.rs
- Create: src/inventory/model.rs
- Create: src/inventory/repository.rs
- Create: src/inventory/service.rs
- Create: src/application/adjust_inventory.rs
- Modify: src/application/mod.rs
- Test: tests/inventory.rs

**Interfaces:**
- Produces: InventoryAvailability { inventory_item_id, inventory_level_id, stocked_quantity, reserved_quantity, available_quantity }
- Produces: inventory::repository::availability_for_variant(...)
- Produces: application::adjust_inventory(AppState, AuthContext, AdjustInventoryInput) -> Result<InventoryLevel, AppError>

- [ ] **Step 1: Write failing inventory invariant tests**

Cover:
- stocked/reserved cannot be negative,
- available = stocked - reserved,
- adjusting stocked from 10 to 6 while reserved=8 succeeds and yields available=-2,
- adjusting stocked below zero fails,
- no reservation is created by adjustment.

- [ ] **Step 2: Add Inventory SQL to 002_pricing_inventory.sql**

~~~sql
CREATE TABLE inventory_locations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    code text NOT NULL UNIQUE,
    name text NOT NULL,
    active boolean NOT NULL DEFAULT true
);

INSERT INTO inventory_locations (code, name)
VALUES ('MAIN', 'Main');

CREATE TABLE inventory_items (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    variant_id uuid NOT NULL UNIQUE REFERENCES product_variants(id),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE inventory_levels (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    inventory_item_id uuid NOT NULL REFERENCES inventory_items(id),
    location_id uuid NOT NULL REFERENCES inventory_locations(id),
    stocked_quantity bigint NOT NULL DEFAULT 0 CHECK (stocked_quantity >= 0),
    reserved_quantity bigint NOT NULL DEFAULT 0 CHECK (reserved_quantity >= 0),
    UNIQUE (inventory_item_id, location_id)
);

CREATE TABLE inventory_adjustments (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    inventory_item_id uuid NOT NULL REFERENCES inventory_items(id),
    location_id uuid NOT NULL REFERENCES inventory_locations(id),
    delta bigint NOT NULL CHECK (delta <> 0),
    reason text NOT NULL,
    note text,
    actor_id uuid,
    created_at timestamptz NOT NULL DEFAULT now()
);
~~~

Do not add CHECK (reserved_quantity <= stocked_quantity).

- [ ] **Step 3: Implement availability query**

Resolve Variant -> InventoryItem -> MAIN InventoryLevel and compute stocked - reserved in SQL or Rust.

Return NOT_FOUND when a sellable Variant lacks required Inventory provisioning.

- [ ] **Step 4: Implement adjust_inventory transaction**

Core sequence:

~~~rust
let mut tx = state.pool.begin().await?;
let level = inventory::repository::lock_level(&mut tx, input.inventory_item_id, input.location_id).await?;
let new_stocked = level.stocked_quantity
    .checked_add(input.delta)
    .ok_or_else(|| AppError::validation("INVENTORY_QUANTITY_OVERFLOW"))?;
if new_stocked < 0 {
    return Err(AppError::validation("NEGATIVE_STOCK"));
}
inventory::repository::insert_adjustment(&mut tx, ...).await?;
let updated = inventory::repository::set_stocked(&mut tx, level.id, new_stocked).await?;
tx.commit().await?;
~~~

Do not compare new_stocked to reserved_quantity.

- [ ] **Step 5: Test row locking with two adjustments**

Run two concurrent adjustments against one level and assert the final quantity equals the serialized sum, with no lost update.

- [ ] **Step 6: Verify**

~~~bash
cargo test --test inventory
cargo test --test catalog
~~~

Expected: PASS.

- [ ] **Step 7: Commit Tasks 2 and 3**

~~~bash
git add migrations/001_catalog.sql migrations/002_pricing_inventory.sql src/catalog src/pricing src/inventory src/application tests/catalog.rs tests/inventory.rs
git commit -m "feat: add catalog pricing and inventory core"
~~~

---

### Task 4: Actor, Customer, addresses, and explicit dev authentication

**Files:**
- Create: migrations/003_actor_customer.sql
- Create: src/actor/*
- Create: src/customer/*
- Create: src/auth.rs
- Create: src/api/store/me.rs
- Modify: src/api/store/mod.rs
- Test: tests/customer_auth.rs

**Interfaces:**
- Produces: AuthContext { actor_id, actor_kind, auth_subject }
- Produces: CustomerContext { auth, customer_id }
- Produces: extractor AuthenticatedCustomer
- Produces: GET /store/me, GET/POST /store/me/addresses

- [ ] **Step 1: Write failing tests**

Cover:
- registered customer resolves from dev header,
- absent header -> 401,
- unknown subject -> 401,
- inactive Actor -> 401,
- Actor without Customer -> 403 for Store customer endpoints,
- Customer A cannot read Customer B's saved address through any endpoint.

- [ ] **Step 2: Implement 003_actor_customer.sql**

~~~sql
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
~~~

- [ ] **Step 3: Implement the dev-header adapter**

Only activate when AUTH_MODE=dev_header. Use X-Dev-Auth-Subject to resolve an active Actor. If AUTH_MODE is anything else and no production adapter exists, startup must fail clearly rather than silently enabling dev auth.

- [ ] **Step 4: Implement CustomerContext resolution and /store/me**

The client never submits customer_id as authority.

- [ ] **Step 5: Implement address create/list**

POST /store/me/addresses accepts address fields only; customer_id is derived from CustomerContext.

- [ ] **Step 6: Verify**

~~~bash
cargo test --test customer_auth
~~~

Expected: PASS.

- [ ] **Step 7: Commit**

~~~bash
git add migrations/003_actor_customer.sql src/actor src/customer src/auth.rs src/api/store/me.rs tests/customer_auth.rs
git commit -m "feat: add actor customer and auth boundary"
~~~

---

### Task 5: Ops foundation and create_sellable_variant

**Files:**
- Create: src/api/ops/mod.rs
- Create: src/api/ops/catalog.rs
- Create: src/api/ops/inventory.rs
- Create: src/application/create_sellable_variant.rs
- Modify: src/application/adjust_inventory.rs
- Modify: src/auth.rs
- Test: tests/operations.rs

**Interfaces:**
- Produces: OpsCaller { actor: AuthContext }
- Produces: POST /ops/catalog/variants
- Produces: POST /ops/inventory/adjustments
- Produces: CreateSellableVariantInput { product_id, sku, title, amount }
- Produces: provenance field initiator_external_ref as untrusted audit/log context, never authorization

- [ ] **Step 1: Write failing auth tests**

Correct Bearer token + configured service Actor -> accepted.
Missing/wrong Bearer token -> 401.
Payload initiator_external_ref does not alter actor_id or authorization.

- [ ] **Step 2: Implement static service-principal auth**

Validate Authorization: Bearer against NOCODB_SERVICE_TOKEN using exact byte/string comparison suitable for this low-risk V1 service secret. Resolve NOCODB_SERVICE_ACTOR_ID and ensure that Actor is active and kind=service.

- [ ] **Step 3: Write failing create_sellable_variant atomicity test**

Force Inventory insertion to fail after Catalog and Pricing inserts (for example by pre-creating conflicting inventory_item for the variant in a controlled test) and assert no partially provisioned Variant/Price remains after rollback.

- [ ] **Step 4: Implement create_sellable_variant in one transaction**

Sequence:
1. validate Product exists,
2. insert ProductVariant,
3. insert IDR VariantPrice,
4. insert InventoryItem,
5. find MAIN,
6. insert InventoryLevel with zero stock,
7. commit.

Return IDs for variant, price, item, and level.

- [ ] **Step 5: Implement Ops routes**

POST /ops/catalog/variants calls only application::create_sellable_variant.
POST /ops/inventory/adjustments calls only application::adjust_inventory.
API handlers must never call Inventory/Catalog repositories directly.

- [ ] **Step 6: Verify**

~~~bash
cargo test --test operations
~~~

Expected: PASS.

- [ ] **Step 7: Commit**

~~~bash
git add src/api/ops src/application/create_sellable_variant.rs src/application/adjust_inventory.rs src/auth.rs tests/operations.rs
git commit -m "feat: add protected commerce operations"
~~~

---

### Task 6: Cart schema, snapshots, address copy, and cart mutations

**Files:**
- Create: migrations/004_cart.sql
- Create: src/cart/*
- Create: src/application/create_or_get_active_cart.rs
- Create: src/application/add_cart_item.rs
- Create: src/application/set_cart_address.rs
- Create: src/api/store/carts.rs
- Test: tests/cart.rs

**Interfaces:**
- Produces: create_or_get_active_cart(CustomerContext) -> Cart
- Produces: add_cart_item(CustomerContext, cart_id, AddCartItemInput) -> CartItem
- Produces: set_cart_shipping_address(CustomerContext, cart_id, customer_address_id) -> CartAddress
- Produces: Store cart routes from the spec

- [ ] **Step 1: Implement 004_cart.sql after writing failing constraint tests**

~~~sql
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
~~~

- [ ] **Step 2: Implement create_or_get_active_cart safely**

Use INSERT ... ON CONFLICT DO NOTHING against the partial unique index, then SELECT the active cart. Add a concurrency test with two simultaneous calls and assert one row exists.

- [ ] **Step 3: Implement cart mutation locking**

Every add/update/remove/address operation first locks the Cart row FOR UPDATE and verifies:
- caller owns it,
- status=active.

This makes complete_cart's Cart lock the serialization boundary against concurrent cart mutation.

- [ ] **Step 4: Implement add_cart_item snapshot query**

Load active Variant from Catalog and IDR price from Pricing in one statement/snapshot. Check current Inventory availability as advisory validation. Insert the current variant_title, sku, and unit_price snapshot. Do not create a reservation.

When POST adds an already-present Variant, increment quantity but retain the original unit_price/title/SKU snapshot; checkout will detect stale price.

- [ ] **Step 5: Implement PATCH/DELETE item routes**

PATCH sets a positive quantity. Quantity <= 0 returns 422 rather than implicitly deleting.
DELETE explicitly removes the row.

- [ ] **Step 6: Implement shipping-address copy**

Load CustomerAddress by both id and authenticated customer_id; copy scalar values into CartAddress using UPSERT on (cart_id, kind='shipping').

- [ ] **Step 7: Add snapshot tests**

After cart addition:
- edit Variant title/SKU/price,
- assert CartItem snapshot remains unchanged,
- assert no InventoryReservation table/row exists yet.

Also test CustomerAddress edit does not mutate CartAddress.

- [ ] **Step 8: Add cross-customer tests**

Customer B receives 403 when trying to mutate Customer A's cart.

- [ ] **Step 9: Verify**

~~~bash
cargo test --test cart
~~~

Expected: PASS.

- [ ] **Step 10: Commit**

~~~bash
git add migrations/004_cart.sql src/cart src/application/create_or_get_active_cart.rs src/application/add_cart_item.rs src/application/set_cart_address.rs src/api/store/carts.rs tests/cart.rs
git commit -m "feat: add cart lifecycle and snapshots"
~~~

---

### Task 7: Order schema, reservation, and atomic complete_cart

**Files:**
- Create: migrations/005_order_reservation.sql
- Create: src/order/*
- Create: src/application/complete_cart.rs
- Create: src/api/store/orders.rs
- Test: tests/checkout.rs

**Interfaces:**
- Produces: complete_cart(CustomerContext, cart_id) -> Order
- Produces: GET /store/orders and GET /store/orders/:id
- Produces stable conflicts: CART_PRICE_CHANGED, VARIANT_INACTIVE, INSUFFICIENT_INVENTORY

- [ ] **Step 1: Write failing migration and checkout tests**

Cover one order per cart, positive order-item quantity, non-negative prices/totals, and reservation quantity > 0.

- [ ] **Step 2: Implement 005_order_reservation.sql**

~~~sql
CREATE TABLE orders (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    cart_id uuid NOT NULL UNIQUE REFERENCES carts(id),
    customer_id uuid NOT NULL REFERENCES customers(id),
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'cancelled')),
    currency_code text NOT NULL DEFAULT 'IDR' CHECK (currency_code = 'IDR'),
    subtotal bigint NOT NULL CHECK (subtotal >= 0),
    total bigint NOT NULL CHECK (total >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    cancelled_at timestamptz
);

CREATE TABLE order_items (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id uuid NOT NULL REFERENCES orders(id),
    variant_id uuid NOT NULL REFERENCES product_variants(id),
    sku text NOT NULL,
    title text NOT NULL,
    quantity bigint NOT NULL CHECK (quantity > 0),
    unit_price bigint NOT NULL CHECK (unit_price >= 0),
    subtotal bigint NOT NULL CHECK (subtotal >= 0),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE order_addresses (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id uuid NOT NULL REFERENCES orders(id),
    kind text NOT NULL CHECK (kind IN ('shipping', 'billing')),
    recipient_name text NOT NULL,
    phone text,
    address_line_1 text NOT NULL,
    address_line_2 text,
    city text NOT NULL,
    province text NOT NULL,
    postal_code text NOT NULL,
    country_code text NOT NULL,
    UNIQUE (order_id, kind)
);

CREATE TABLE inventory_reservations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    inventory_item_id uuid NOT NULL REFERENCES inventory_items(id),
    location_id uuid NOT NULL REFERENCES inventory_locations(id),
    order_item_id uuid NOT NULL REFERENCES order_items(id),
    quantity bigint NOT NULL CHECK (quantity > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    released_at timestamptz,
    UNIQUE (order_item_id, location_id)
);
~~~

- [ ] **Step 3: Implement complete_cart_once transaction skeleton**

Use exactly one SQLx transaction. Lock Cart first. If completed, load the existing Order by cart_id and return it without creating anything.

- [ ] **Step 4: Lock mutable Catalog/Pricing rows during checkout validation**

For all cart variant IDs in sorted UUID order:
- SELECT ProductVariant ... FOR SHARE,
- SELECT VariantPrice ... FOR SHARE.

This prevents a direct NocoDB UPDATE of active/price from racing between validation and commit.

If any Variant is inactive -> VARIANT_INACTIVE.
If current IDR amount differs from CartItem.unit_price -> CART_PRICE_CHANGED with variant_id, old_price, current_price.

- [ ] **Step 5: Lock MAIN InventoryLevels in deterministic order**

Resolve every cart Variant to its MAIN InventoryLevel, sort by inventory_level.id, then SELECT ... FOR UPDATE in that order. Recompute availability after lock acquisition.

Reject any insufficient item with INSUFFICIENT_INVENTORY before inserting Order.

- [ ] **Step 6: Insert Order and immutable snapshots**

Calculate subtotal with checked integer multiplication/addition. total=subtotal.
Insert OrderItems from CartItem snapshots, not current Catalog title/SKU.
Copy CartAddress into OrderAddress.

- [ ] **Step 7: Reserve inventory atomically**

For each OrderItem:
- insert inventory_reservations,
- UPDATE inventory_levels SET reserved_quantity = reserved_quantity + quantity.

Then mark Cart completed and set completed_at=now(); commit.

- [ ] **Step 8: Write rollback test**

Inject a controlled failure after Order insertion but before reservation update and assert after rollback:
- no Order,
- no OrderItem,
- no reservation,
- reserved_quantity unchanged,
- Cart still active.

Use a test-only hook only if necessary; prefer a database constraint violation that naturally occurs at the desired point.

- [ ] **Step 9: Write stale-price race test**

Hold checkout after its Cart lock, update price on a second connection, and verify the row lock ordering makes one operation wait. Accepted checkout must either observe old price while holding a lock that delays the admin change, or observe new price and reject CART_PRICE_CHANGED; it must not commit against an unvalidated price.

- [ ] **Step 10: Verify**

~~~bash
cargo test --test checkout
~~~

Expected: PASS.

- [ ] **Step 11: Commit**

~~~bash
git add migrations/005_order_reservation.sql src/order src/application/complete_cart.rs src/api/store/orders.rs tests/checkout.rs
git commit -m "feat: add atomic checkout and reservations"
~~~

---

### Task 8: Checkout concurrency, retry classifier, and order cancellation

**Files:**
- Create: src/application/retry.rs
- Create: src/application/cancel_order.rs
- Create: src/api/ops/orders.rs
- Modify: src/application/complete_cart.rs
- Test: tests/checkout.rs
- Create: tests/cancellation.rs

**Interfaces:**
- Produces: is_retryable_db_error(&sqlx::Error) -> bool
- Produces: complete_cart with bounded whole-operation retry
- Produces: cancel_order(OpsCaller, order_id) -> Order
- Produces: POST /ops/orders/:id/cancel

- [ ] **Step 1: Write retry-classifier unit tests**

40P01 and 40001 -> retryable.
Unique violation, FK violation, CHECK violation, and domain AppError -> not retryable.

- [ ] **Step 2: Implement bounded retry around the whole complete_cart_once operation**

Use max retries from Config, default 2. Re-run from BEGIN; never retry CART_PRICE_CHANGED, INSUFFICIENT_INVENTORY, VARIANT_INACTIVE, FORBIDDEN, or validation errors.

- [ ] **Step 3: Write the one-unit/two-carts concurrency test**

Seed:
- stocked=1,
- reserved=0,
- two Customers,
- two active carts, quantity=1.

Start complete_cart concurrently with a barrier. Assert:
- exactly one succeeds,
- exactly one returns INSUFFICIENT_INVENTORY,
- exactly one Order exists,
- exactly one active reservation exists,
- reserved_quantity=1,
- available_quantity=0.

- [ ] **Step 4: Write repeat-complete idempotency test**

Call complete_cart twice for one Cart. Assert same order_id and reservation count remains one.

- [ ] **Step 5: Write failing cancellation tests**

Cover:
- pending -> cancelled,
- reserved decreases exactly once,
- stocked unchanged,
- reservation released_at set,
- repeated cancellation is success/no-op,
- caller is authenticated Ops service,
- cannot produce negative reserved quantity.

- [ ] **Step 6: Implement cancel_order transaction**

Lock Order first. If already cancelled, return it.
Load active reservations and their InventoryLevels.
Lock InventoryLevels in deterministic ID order.
For each active reservation:
- decrement reserved_quantity by reservation.quantity,
- set released_at=now().
Mark Order cancelled and cancelled_at=now().
Commit.

- [ ] **Step 7: Verify**

~~~bash
cargo test --test checkout
cargo test --test cancellation
~~~

Expected: PASS.

- [ ] **Step 8: Commit**

~~~bash
git add src/application/retry.rs src/application/complete_cart.rs src/application/cancel_order.rs src/api/ops/orders.rs tests/checkout.rs tests/cancellation.rs
git commit -m "feat: harden checkout and cancellation"
~~~

---

### Task 9: NocoDB PostgreSQL boundary and operator read model

**Files:**
- Create: ops/sql/nocodb_grants.sql
- Create: ops/README.md
- Create: tests/nocodb_role.rs
- Modify: migrations only if an ops view belongs in a versioned migration; otherwise create a dedicated versioned migration before release

**Interfaces:**
- Produces: documented DBA setup for commerce_nocodb
- Produces: ops_inventory_summary SELECT-only read model
- Produces: acceptance test for direct-vs-command mutation boundary

- [ ] **Step 1: Write the role acceptance test**

Using a restricted PostgreSQL role, assert:
- products maintained fields can be updated,
- product_variants title/sku/active can be updated,
- existing variant_prices.amount can be updated,
- product_variants INSERT fails,
- variant_prices INSERT fails,
- inventory_levels UPDATE fails,
- orders UPDATE fails,
- inventory_reservations INSERT fails,
- actors security mappings cannot be updated,
- DDL fails.

If CI cannot CREATE ROLE, mark this test with an explicit required environment flag and run it in a dedicated CI job using the PostgreSQL superuser.

- [ ] **Step 2: Write nocodb_grants.sql**

Assume the role already exists; do not embed a password.

~~~sql
GRANT CONNECT ON DATABASE CURRENT_DATABASE TO commerce_nocodb;
GRANT USAGE ON SCHEMA public TO commerce_nocodb;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO commerce_nocodb;

GRANT INSERT (title, description, status),
      UPDATE (title, description, status)
ON products TO commerce_nocodb;

GRANT UPDATE (sku, title, active)
ON product_variants TO commerce_nocodb;

GRANT UPDATE (amount)
ON variant_prices TO commerce_nocodb;

GRANT UPDATE (email, phone, first_name, last_name)
ON customers TO commerce_nocodb;

GRANT INSERT, UPDATE
ON customer_addresses TO commerce_nocodb;

REVOKE INSERT, DELETE ON product_variants FROM commerce_nocodb;
REVOKE INSERT, DELETE ON variant_prices FROM commerce_nocodb;
REVOKE INSERT, UPDATE, DELETE ON inventory_levels FROM commerce_nocodb;
REVOKE INSERT, UPDATE, DELETE ON inventory_reservations FROM commerce_nocodb;
REVOKE INSERT, UPDATE, DELETE ON orders FROM commerce_nocodb;
REVOKE ALL ON actors FROM commerce_nocodb;
GRANT SELECT ON actors TO commerce_nocodb;
~~~

Adjust syntax for PostgreSQL column-level INSERT privileges during implementation if CURRENT_DATABASE cannot be used directly in GRANT; keep the privilege matrix unchanged.

- [ ] **Step 3: Add ops_inventory_summary as a versioned PostgreSQL view**

The view may join Product, Variant, InventoryItem, MAIN InventoryLevel, and Price. Grant SELECT only.

- [ ] **Step 4: Document the NocoDB operational requirements learned from the Nomad spike**

ops/README.md must state:
- NocoDB metadata PostgreSQL requires durable storage appropriate to Nomad topology; allocation-local Docker volumes are insufficient for allocation replacement.
- after any noko-rs schema migration, trigger NocoDB Metadata Sync before operator use.
- NocoDB service Bearer credential authenticates the service principal; human user identity from webhook payload is provenance only.
- NocoDB is not part of Store readiness.

- [ ] **Step 5: Verify**

~~~bash
cargo test --test nocodb_role
~~~

Expected: PASS in the privileged CI job.

- [ ] **Step 6: Commit**

~~~bash
git add ops tests/nocodb_role.rs migrations
git commit -m "ops: enforce nocodb database boundary"
~~~

---

### Task 10: Persistent architecture docs and full V1 acceptance

**Files:**
- Create: docs/architecture.md
- Create: docs/modules/catalog.md
- Create: docs/modules/pricing.md
- Create: docs/modules/inventory.md
- Create: docs/modules/actor-customer.md
- Create: docs/modules/cart.md
- Create: docs/modules/order.md
- Modify: README.md if/when it exists

**Interfaces:**
- Produces: bounded implementation context for future human/AI work
- No new runtime API

- [ ] **Step 1: Write concise module docs**

Each file answers:
- what the module owns,
- what it exposes,
- invariants,
- what it explicitly does not own,
- which application operations coordinate it.

Do not duplicate the whole approved spec.

- [ ] **Step 2: Add architecture.md**

Summarize:
- modular-monolith dependency rule,
- transaction ownership,
- Store vs Ops boundaries,
- NocoDB direct-edit vs command path,
- external-side-effect breakpoint for future Payment.

- [ ] **Step 3: Run the complete test suite**

~~~bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
~~~

Expected: PASS.

- [ ] **Step 4: Run explicit V1 acceptance scenarios**

Verify with real PostgreSQL:
1. Create Product.
2. create_sellable_variant provisions Variant + Price + InventoryItem + MAIN level.
3. adjust_inventory to 10.
4. Create registered Actor + Customer + address.
5. Create/get Cart.
6. Add quantity 2; verify reserved=0.
7. Set shipping address.
8. complete_cart; verify Order snapshots and reserved=2.
9. Retry complete_cart; same Order.
10. Cancel Order; reserved=0, stocked=10.
11. Change stocked to 0 while a separate test reservation exceeds stock; physical adjustment is accepted but next checkout fails.
12. Run concurrent one-unit checkout; exactly one succeeds.
13. Change price after add-to-cart; checkout returns CART_PRICE_CHANGED.

- [ ] **Step 5: Verify architecture exclusions**

Search dependency tree and source:
- no Redis client,
- no queue/worker framework,
- no payment SDK,
- no generic workflow/saga library,
- no direct API -> repository cross-module business mutation.

Use:

~~~bash
cargo tree
rg "redis|bull|payment|workflow|saga" Cargo.toml src
~~~

Review matches manually; documentation words are acceptable, runtime dependencies are not.

- [ ] **Step 6: Commit**

~~~bash
git add docs README.md
git commit -m "docs: document v1 commerce module boundaries"
~~~

- [ ] **Step 7: Final verification before execution completion**

Use the verification-before-completion skill. Capture fresh output from fmt, clippy, full tests, and git status before claiming the branch is complete.

---

## Plan self-review notes

### Spec coverage
Covered: Foundation, Catalog, Pricing, Inventory, Actor/Customer, Store auth adapter, Ops service auth, create_sellable_variant, Cart, snapshots, stale-price handling, complete_cart atomicity, row locking, reservations, idempotency, cancellation, retry classification, NocoDB grants/read models, health/readiness, structured tracing, real PostgreSQL tests, persistent architecture docs.

Explicitly not implemented because the spec defers them: Payment, Fulfillment execution, POINT, discounts/promotions, tax, paid shipping/rate selection, multi-location allocation, Operation audit table, Redis/Event Bus/Worker/outbox/workflow engine.

### Corrected stale spec example
Section 32's old illustrative inventory invariant contradicted the approved Sections 8.3, 14, and 27. Task 1 corrects only that stale example; the implementation follows the approved rule: physical adjustment may make stocked < reserved, but stocked must remain >= 0 and new reservations require sufficient availability.

### Type consistency
All external IDs are UUID. Money/quantity values are i64/BIGINT. Application operations take AppState plus authenticated context. Module writes accept a mutable PostgreSQL connection/transaction rather than owning transactions themselves.

### Review-focus coverage
- Admin price/Variant race: Task 7 Steps 4 and 9.
- One-unit concurrent checkout: Task 8 Step 3.
- Physical shortage after adjustment: Task 3 Step 1 and Task 10 acceptance scenario.
- Repeat checkout: Task 8 Step 4.
- Cross-customer access: Task 6 Step 8.

---

## Execution handoff

Recommended execution method: **Subagent-driven**, because the plan has ten reviewable tasks with explicit inter-task interfaces, and mistakes in transaction/locking behavior are expensive enough to justify a fresh implementation context and reviewer per task.
