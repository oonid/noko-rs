# PostgreSQL-Backed Rust Commerce Modular Monolith

**Status:** Approved conversational design, formal specification for review  
**Date:** 2026-09-20  
**Scope:** Initial commerce architecture through Order placement, with Payment as the nearest post-V1 increment

## 1. Purpose

Build a small, useful headless-commerce backend in Rust that can grow over a long period without requiring an early microservice architecture or a large custom admin application.

The design borrows architectural guidance from Medusa, especially:

- domain-oriented modules,
- application/workflow orchestration across modules,
- inventory reservation semantics,
- transactional snapshots for orders,
- explicit evolution toward durable workflows only when external side effects require them.

It does **not** aim to reproduce Medusa's framework, API compatibility, module-link machinery, workflow engine, or feature surface.

The initial operational UI is NocoDB over PostgreSQL. PostgreSQL remains the system of record; Rust remains the owner of commerce behavior; NocoDB is a privileged human-facing operational surface.

## 2. Primary goals

1. Reach a usable `catalog -> inventory -> customer -> cart -> order` flow quickly.
2. Keep each implementation increment small enough for short, bounded AI-assisted coding sessions.
3. Preserve stable domain boundaries so later Payment, Fulfillment, POINT settlement, richer Pricing, promotions, returns, and integrations can be added without collapsing existing modules into a monolith of business logic.
4. Use PostgreSQL constraints, transactions, locks, and snapshots aggressively while the system remains a single-database modular monolith.
5. Avoid building infrastructure for failure modes that do not yet exist.
6. Avoid building a custom backoffice while NocoDB can provide the necessary operator experience safely.

## 3. Non-goals for the first commerce milestone

The first commerce milestone does not implement:

- guest checkout,
- Payment,
- Fulfillment,
- POINT balances or mixed settlement,
- discounts or promotions,
- tax calculation,
- paid shipping or shipping-rate selection,
- multi-location allocation,
- event bus or outbox,
- durable workflow/saga infrastructure,
- background workers,
- generic policy engine,
- sophisticated RBAC,
- microservices,
- distributed cache,
- search engine,
- generalized reservation expiry.

These are explicit deferrals rather than accidental omissions.

## 4. Architectural style

The application is a **modular monolith**:

```text
one Rust binary
one Rust crate initially
one PostgreSQL database
one NocoDB deployment
```

Conceptual layering:

```text
Storefront          NocoDB Interfaces          AI / Services
    |                      |                        |
    | /store               | workflow -> /ops      | /ops
    +----------------------+------------------------+
                           |
                           v
                    Rust / Axum HTTP
                           |
                    authentication
                    authorization
                           |
                           v
                  application operations
                           |
          +----------------+----------------+
          |                |                |
          v                v                v
       Catalog          Inventory        Customer
          |                |                |
          +----------------+----------------+
                           |
                       Cart / Order
                           |
                           v
                       PostgreSQL
```

### 4.1 Dependency rule

Commerce modules are peers. A commerce module does not call another commerce module directly.

Cross-module behavior belongs in the application layer.

```text
BAD
Order -> SQL UPDATE inventory_levels

GOOD
application::cancel_order
    -> Order
    -> Inventory
```

### 4.2 Module ownership rule

A module exclusively owns mutations to its domain state.

Cross-module PostgreSQL foreign keys are permitted for referential integrity because this is one PostgreSQL-backed modular monolith. A foreign key does not transfer behavioral ownership.

### 4.3 Read/write asymmetry

Writes obey strict module ownership.

Read/query code may join across module-owned tables for read models, reporting, NocoDB operator views, and response composition, provided it does not implement business mutations.

This is intentionally lighter than Medusa's strong module isolation and Module Links. Medusa's current documentation states that modules cannot access other modules' resources and cross-module functionality should be composed with workflows; this project adopts the same behavioral separation without reproducing the framework machinery.

Reference: https://docs.medusajs.com/learn/fundamentals/modules/isolation

## 5. Initial domain modules

### 5.1 Catalog

Owns:

- products,
- product variants,
- V1 fixed IDR price data.

Does not own:

- stock quantities,
- carts,
- orders,
- payment state.

### 5.2 Inventory

Owns:

- inventory items,
- locations,
- levels,
- adjustments,
- reservations,
- availability semantics.

Does not own:

- product descriptions,
- carts,
- orders,
- fulfillment execution.

### 5.3 Customer

Owns:

- customer commerce profile,
- saved customer addresses.

A Customer is not the same concept as an Actor.

### 5.4 Cart

Owns:

- active/completed cart state,
- cart items,
- cart address snapshots.

Does not reserve inventory.

### 5.5 Order

Owns:

- order lifecycle state,
- order item snapshots,
- order address snapshots,
- canonical commercial totals in IDR.

Does not own payment state or fulfillment state.

### 5.6 Actor

Represents a principal capable of acting:

- human,
- agent,
- service.

Actor expresses identity/provenance, not commerce-customer status and not authority by itself.

### 5.7 Operation

Deferred until command auditing/AI operations justify it.

Eventually owns:

- operation requests,
- execution status,
- approval state,
- caller and initiator provenance,
- operation payload/result/error metadata.

It never duplicates authoritative commerce state.

## 6. V1 business policies

These are deliberate V1 restrictions, not permanent domain truths:

- registered customers only,
- exactly one active/default inventory location,
- IDR only,
- one fixed price per variant,
- no discount/promotion logic,
- free shipping,
- no tax model,
- one active cart per customer,
- no inventory reservation while merely in cart,
- order placement reserves inventory,
- order cancellation releases inventory,
- no reservation TTL/expiry,
- order state is only `pending` or `cancelled`,
- Payment is the next increment after Order.

## 7. Pricing and future POINT settlement

### 7.1 V1

V1 pricing is deliberately simple:

```text
currency = IDR
one fixed price per variant
no discounts
no price lists
no customer-group pricing
```

The V1 fixed price may be stored in a small `variant_prices` table rather than directly on `product_variants`, preserving a clean extraction point for a future Pricing module without implementing that module now.

### 7.2 POINT

POINT is a future internal value unit/tender, **not** an ISO fiat currency.

Future settlement may support:

- IDR only,
- POINT only,
- mixed IDR + POINT.

The order's commercial value remains canonical in IDR. Payment/settlement records describe how the IDR-denominated obligation was settled.

A future POINT design is expected to introduce an account/ledger model and an explicit conversion policy, for example `1 POINT = N IDR`. That design is intentionally deferred until Payment exists.

## 8. V1 relational data model

The first order-capable schema is approximately 17 small tables.

### 8.1 Catalog

```text
products
--------
id
title
description
status
created_at
updated_at

product_variants
----------------
id
product_id          FK -> products
sku                 UNIQUE
title
active
created_at
updated_at

variant_prices
--------------
id
variant_id          FK -> product_variants
currency_code       V1 = 'IDR'
amount
created_at
updated_at

UNIQUE (variant_id, currency_code)
```

V1 policy: exactly one IDR price per active variant.

### 8.2 Inventory

```text
inventory_locations
-------------------
id
code                UNIQUE
name
active

inventory_items
---------------
id
variant_id          FK -> product_variants
sku
created_at

UNIQUE (variant_id)

inventory_levels
----------------
id
inventory_item_id   FK -> inventory_items
location_id         FK -> inventory_locations
stocked_quantity
reserved_quantity

UNIQUE (inventory_item_id, location_id)

inventory_adjustments
---------------------
id
inventory_item_id
location_id
delta
reason
note
actor_id NULLABLE
created_at

inventory_reservations
----------------------
id
inventory_item_id
location_id
order_item_id
quantity
created_at
released_at NULLABLE

UNIQUE (order_item_id, location_id)
```

Availability:

```text
available_quantity = stocked_quantity - reserved_quantity
```

Required invariants include:

```text
stocked_quantity >= 0
reserved_quantity >= 0
reserved_quantity <= stocked_quantity
```

The one-location restriction is an application policy. The schema still models Location and InventoryLevel so multi-location support does not require collapsing stock onto ProductVariant.

### 8.3 Actor and Customer

```text
actors
------
id
kind                human | agent | service
auth_subject
display_name
active
created_at

UNIQUE (auth_subject)

customers
---------
id
actor_id            FK -> actors
email
phone
first_name
last_name
created_at
updated_at

UNIQUE (actor_id)
UNIQUE (email)

customer_addresses
------------------
id
customer_id         FK -> customers
label
recipient_name
phone
address_line_1
address_line_2
city
province
postal_code
country_code
is_default
created_at
updated_at
```

For V1, a customer has an Actor because checkout is registered-only. Human operators, agents, and services may have Actors without Customers.

### 8.4 Cart

```text
carts
-----
id
customer_id         FK -> customers
currency_code       V1 = 'IDR'
status              active | completed
created_at
updated_at
completed_at NULLABLE

cart_items
----------
id
cart_id             FK -> carts
variant_id          FK -> product_variants
quantity
unit_price
created_at
updated_at

UNIQUE (cart_id, variant_id)

cart_addresses
--------------
id
cart_id             FK -> carts
kind                shipping | billing
recipient_name
phone
address_line_1
address_line_2
city
province
postal_code
country_code

UNIQUE (cart_id, kind)
```

V1 requires one active cart per customer, preferably enforced with a PostgreSQL partial unique index.

`cart_items.unit_price` is a checkout snapshot. A later catalog price edit does not silently change an existing cart. Future repricing behavior, if needed, must be explicit.

### 8.5 Order

```text
orders
------
id
cart_id             FK -> carts
customer_id         FK -> customers
status              pending | cancelled
currency_code       V1 = 'IDR'
subtotal
total
created_at
cancelled_at NULLABLE

UNIQUE (cart_id)

order_items
-----------
id
order_id            FK -> orders
variant_id          FK -> product_variants
sku
title
quantity
unit_price
subtotal
created_at

order_addresses
---------------
id
order_id            FK -> orders
kind                shipping | billing
recipient_name
phone
address_line_1
address_line_2
city
province
postal_code
country_code

UNIQUE (order_id, kind)
```

Order items retain both a reference (`variant_id`) and historical snapshots (`sku`, `title`, `unit_price`).

V1 total calculation:

```text
subtotal = SUM(order_item.unit_price * order_item.quantity)
total    = subtotal
```

because shipping is free and tax is not modeled in V1.

There is intentionally no `payment_status`, `fulfillment_status`, `payment_id`, `shipment_id`, `points_used`, or `discount_total` in V1 Order.

## 9. Snapshot semantics

Mutable maintained data must not rewrite historical transaction truth.

```text
Current data                 Snapshot
------------                 --------
Variant title      ------->  OrderItem.title
SKU                ------->  OrderItem.sku
Variant price      ------->  CartItem.unit_price
                               OrderItem.unit_price
CustomerAddress    ------->  CartAddress
CartAddress        ------->  OrderAddress
```

Editing a saved customer address after checkout does not mutate the CartAddress or OrderAddress snapshots.

Editing catalog title/SKU/price after order creation does not rewrite historical OrderItems.

## 10. Inventory semantics

Inventory distinguishes:

1. physical quantity (`stocked_quantity`),
2. committed but still physically present quantity (`reserved_quantity`),
3. derived availability (`stocked - reserved`).

An inventory adjustment changes physical stock.

A reservation changes availability without changing physical stock.

Examples:

```text
receive shipment      adjustment +20
damaged stock         adjustment -2
manual correction     adjustment +/-N

order placement       reservation +N
order cancellation    reservation release
future fulfillment    consume stocked + reserved quantities
```

This follows Medusa's current reservation model: order placement through cart completion creates reservations, reservations raise `reserved_quantity` while leaving `stocked_quantity` unchanged, cancellation releases reservations, and fulfillment consumes both stocked and reserved quantities.

References:

- https://docs.medusajs.com/resources/commerce-modules/inventory/reservations-lifecycle
- https://docs.medusajs.com/resources/commerce-modules/inventory/inventory-in-flows

## 11. Application operations

The application layer is the lightweight equivalent of Medusa workflow orchestration for V1.

Each operation is a small Rust unit with explicit input, validation, transaction boundary, module calls, and result.

Initial operations:

```text
create_or_get_active_cart
add_cart_item
update_cart_item
remove_cart_item
set_cart_shipping_address
adjust_inventory
complete_cart
cancel_order
```

### 11.1 Module operations vs application operations

Example module operations:

```text
Catalog.get_variant
Inventory.get_availability
Inventory.adjust
Inventory.reserve
Inventory.release
Cart.add_or_update_item
Order.create
```

Example application operation:

```text
complete_cart
    -> Customer
    -> Cart
    -> Inventory
    -> Order
```

The application layer owns the cross-module transaction boundary.

## 12. Cart and checkout behavior

### 12.1 Add cart item

`add_cart_item`:

1. derives the customer from authenticated context,
2. verifies cart ownership and active state,
3. loads active variant and current fixed IDR price from Catalog,
4. checks current Inventory availability as an advisory validation,
5. writes/updates CartItem with a unit-price snapshot.

Adding an item to a cart does **not** reserve inventory.

Two customers may therefore both temporarily hold cart quantities whose sum exceeds currently available stock. Final allocation occurs only at `complete_cart`.

### 12.2 Shipping address

`set_cart_shipping_address` validates that the selected saved CustomerAddress belongs to the authenticated Customer and copies it into CartAddress.

It does not retain only a mutable reference to CustomerAddress.

## 13. Complete cart: core V1 transaction

`complete_cart(customer_id, cart_id) -> order_id` is the critical V1 operation.

The request never trusts client-supplied prices, totals, inventory quantities, or customer identifiers as authoritative commerce data.

Conceptual transaction:

```text
BEGIN

1. SELECT cart FOR UPDATE

2. If cart is already completed:
       load order by cart_id
       return existing order

3. Validate:
       authenticated customer owns cart
       cart is active
       cart has items
       shipping address exists

4. Load inventory levels
       lock rows in deterministic ID order

5. Validate:
       available_quantity >= requested quantity
       for every cart item

6. Create Order

7. Snapshot:
       customer reference
       item SKU/title/unit price
       shipping address
       subtotal/total

8. Create InventoryReservations

9. Increase InventoryLevel.reserved_quantity

10. Mark Cart completed

COMMIT

return order_id
```

### 13.1 Atomicity invariant

A successful `complete_cart` means all required Order, snapshot, reservation, and Cart-completion changes commit together.

If any step fails, none commit.

While all consequential effects live in PostgreSQL, one ACID transaction is preferred over compensation/saga machinery.

### 13.2 Idempotency

Two safeguards are mandatory:

1. lock the Cart row during completion,
2. enforce `orders.cart_id UNIQUE`.

Repeated completion for the same cart returns the existing order instead of creating a second order.

The current Medusa `completeCartWorkflow` also explicitly protects cart-completion idempotency/concurrency and creates inventory reservations during order placement. This project adopts the invariant while using PostgreSQL locking/uniqueness instead of Medusa's workflow engine.

References:

- https://docs.medusajs.com/resources/storefront-development/checkout/complete-cart
- https://github.com/medusajs/medusa/blob/48a8812735f6630bcc12b3997b6d1d1f559cd492/packages/core/core-flows/src/cart/workflows/complete-cart.ts

### 13.3 Deterministic inventory locking

When multiple inventory rows must be locked, acquire them in a deterministic order, for example by inventory-level ID.

This reduces avoidable deadlocks for concurrent orders containing overlapping variants.

## 14. Inventory adjustment command

Inventory quantities are not ordinary NocoDB-editable fields.

`adjust_inventory(inventory_item_id, location_id, delta, reason, actor)` runs in a transaction:

```text
BEGIN

SELECT inventory_level FOR UPDATE
validate resulting quantity
INSERT inventory_adjustment
UPDATE inventory_level.stocked_quantity

COMMIT
```

V1 rejects an adjustment that would violate:

```text
stocked_quantity >= reserved_quantity
```

This preserves committed inventory.

## 15. Order cancellation

V1 `cancel_order(actor, order_id, reason?)`:

```text
BEGIN

lock order

if already cancelled:
    return success

validate order is cancellable
load active reservations
lock affected inventory levels deterministically
release reservations
decrease reserved quantities
mark order cancelled

COMMIT
```

Cancellation is idempotent. Repeating it does not release inventory twice.

Later Payment and Fulfillment extend application-level cancellation orchestration rather than moving their behavior into Order.

## 16. State machines

### 16.1 Cart

```text
active -> completed
```

### 16.2 Order

```text
pending -> cancelled
```

There is intentionally no `completed` order state in the first milestone. Commerce-level completion should be defined only after Payment and Fulfillment semantics exist.

### 16.3 Reservation

Conceptually:

```text
active -> released
```

V1 reservation begins at successful order creation and ends at cancellation. Fulfillment consumption is a future transition.

There is no reservation TTL or abandoned-cart reservation.

## 17. API boundaries

### 17.1 Store API

Conceptual surface:

```text
GET    /store/products
GET    /store/products/:id

GET    /store/me
GET    /store/me/addresses
POST   /store/me/addresses

GET    /store/carts/current
POST   /store/carts
POST   /store/carts/:id/items
PATCH  /store/carts/:id/items/:item_id
DELETE /store/carts/:id/items/:item_id
POST   /store/carts/:id/shipping-address
POST   /store/carts/:id/complete

GET    /store/orders
GET    /store/orders/:id
```

Public product reads may be unauthenticated. Customer/cart/order state is authenticated.

The client does not supply `customer_id` as authority. Customer identity is derived from authentication context.

### 17.2 Ops API

Use typed domain endpoints rather than one generic operation-switch endpoint.

V1 examples:

```text
POST /ops/inventory/adjustments
POST /ops/orders/:id/cancel
```

Future examples:

```text
POST /ops/payments/:id/verify
POST /ops/payments/:id/refund
POST /ops/fulfillments
```

Typed endpoints provide clearer schemas, permissions, validation, and versioning.

## 18. Authentication, Actor, Customer, and provenance

Authentication is an adapter boundary, not a commerce-module concern.

```text
JWT / session / API key
        |
        v
Authentication adapter
        |
        v
AuthContext
    actor_id
    actor_kind
    auth_subject
        |
        v
application operation
```

Commerce code does not know how JWT parsing, password hashing, OAuth, NocoDB login, or agent API keys work.

### 18.1 Customer vs Actor

```text
Actor    = who can act
Customer = who participates as a buyer in commerce
```

Examples:

```text
registered shopper  Actor + Customer
human operator      Actor
AI inventory agent  Actor
integration service Actor
```

### 18.2 Authenticated caller vs initiator

For NocoDB-triggered operations, the Rust API authenticates a NocoDB service principal. The human user reported by the NocoDB workflow is provenance unless independently mapped/verified.

```text
authenticated_actor = nocodb-service
initiator_external_ref = NocoDB user identity
channel = nocodb
```

Do not treat a payload field such as `requested_by = admin@example.com` as proof of authorization.

## 19. NocoDB role

NocoDB is:

- a human operational UI,
- a safe maintained-data editor,
- a command launcher through workflows.

NocoDB is not the commerce application layer and does not own the database schema.

### 19.1 External PostgreSQL source

Use NocoDB's external PostgreSQL data source with schema editing disabled.

Current NocoDB documentation states that external-source data editing and schema editing are separate permissions; schema editing is disabled by default, while views and virtual columns can still be created without modifying the source schema.

Reference: https://nocodb.com/docs/product/integrations/data-sources/connect-to-data-source

### 19.2 Safe direct CRUD vs business commands

Direct PostgreSQL edits through NocoDB are permitted for maintained facts such as:

- product title/description/status,
- variant metadata/SKU/active flag,
- V1 fixed price,
- selected customer profile fields,
- customer saved addresses.

Business transitions go through Rust:

- inventory adjustment,
- inventory reservation/release,
- order cancellation,
- future payment verification/refund,
- future fulfillment operations.

Rule:

> Editing describes maintained data; commands perform business actions.

### 19.3 NocoDB workflows

NocoDB Interfaces may expose actions such as `Adjust Inventory` or `Cancel Order`. A button-triggered workflow calls the typed Rust `/ops` endpoint using a service credential.

NocoDB's current workflow documentation supports button-triggered workflows that receive the clicked record/user context and HTTP Request actions with configurable method, headers, and request body.

References:

- https://nocodb.com/docs/workflows/nodes/trigger-nodes/button-clicked
- https://nocodb.com/docs/workflows/nodes/action-nodes/http-request

### 19.4 Attachments

For future commands that include attachments, NocoDB may collect the attachment and pass a reference/URL plus metadata to Rust.

Rust remains responsible for validation, durable-copy policy, checksum/provenance, and authoritative attachment references. Do not make transient NocoDB attachment URLs the long-term commerce record.

## 20. PostgreSQL and NocoDB governance

Use separate PostgreSQL principals:

```text
commerce_app      Rust runtime
commerce_nocodb   NocoDB
```

A separate migration/DDL principal may be introduced when deployment warrants it.

NocoDB never receives a superuser credential and never shares the Rust runtime credential.

### 20.1 Exposure classes

Every table is classified as:

```text
A. directly editable
B. read-only
C. hidden
```

Recommended V1 posture:

| Table | NocoDB posture |
|---|---|
| products | editable |
| product_variants | editable |
| variant_prices | editable |
| inventory_locations | mostly read-only |
| inventory_items | limited metadata edit |
| inventory_levels | read-only |
| inventory_adjustments | read-only |
| inventory_reservations | read-only |
| customers | selected fields editable |
| customer_addresses | editable |
| carts | read-only |
| cart_items | read-only || cart_addresses | read-only |
| orders | read-only |
| order_items | read-only |
| order_addresses | read-only |
| actors | hidden or read-only |
| operation_requests | read-only once introduced |

### 20.2 PostgreSQL is authoritative for access control

NocoDB's UI permissions improve operator experience but do not define the final integrity boundary.

PostgreSQL grants should prevent NocoDB from mutating transactional state even if an Interface is accidentally misconfigured.

Where useful, use column-level privileges for sensitive fields such as `actor_id`.

### 20.3 Prefer deactivation to destructive deletion

For catalog entities referenced by historical commerce data, prefer status/active flags over hard deletion.

NocoDB's role should generally not have DELETE privileges on core catalog and transactional tables.

### 20.4 Operator read models

When raw relational navigation becomes awkward, introduce PostgreSQL views such as:

```text
ops_order_summary
ops_inventory_summary
ops_customer_order_history
```

NocoDB receives SELECT-only access to those views.

Do not create a large catalog of read views before operator needs justify them.

## 21. Operation audit and AI controls

`operation_requests` is deferred until there are multiple operator/agent command sources that benefit from durable audit and approval state.

When introduced, Rust creates and owns the record.

Conceptual shape:

```text
operation_requests
------------------
id
operation
resource_type
resource_id
authenticated_actor_id
initiated_by_actor_id NULLABLE
initiator_external_ref NULLABLE
channel                storefront | nocodb | agent | service
payload
attachments
status
result
error
created_at
started_at
completed_at
```

NocoDB reads operation history but does not insert command records directly.

### 21.1 AI agents

AI agents call Rust APIs directly and never write PostgreSQL directly or use NocoDB as their application API.

Future risk-based authorization:

```text
authenticated actor
      |
      v
authorization
      |
      v
risk evaluation
   /       \
low        sensitive
 |            |
execute    approval required
```

Human/agent identity describes provenance, not authority. Roles/permissions remain a separate concern.

## 22. Error model

Use a small stable error taxonomy shared across Store and Ops APIs.

| Category | Example | Typical HTTP mapping |
|---|---|---:|
| validation | quantity <= 0 | 422 |
| not found | unknown variant | 404 |
| forbidden | cart owned by another customer | 403 |
| conflict | insufficient inventory / invalid state transition | 409 |
| infrastructure | DB unavailable / unexpected SQL failure | 500/503 |

Responses should distinguish retryability where useful.

Example domain rejection:

```json
{
  "code": "INSUFFICIENT_INVENTORY",
  "retryable": false,
  "details": {
    "variant_id": "var_...",
    "requested": 5,
    "available": 3
  }
}
```

Example transient infrastructure failure:

```json
{
  "code": "DATABASE_UNAVAILABLE",
  "retryable": true
}
```

## 23. Retry policy

Automatic retries are narrow and only apply to known transient infrastructure/concurrency failures such as deadlock-victim or serialization errors.

Retry the **whole idempotent application operation**, not an arbitrary SQL fragment.

Domain conflicts such as insufficient inventory are never blindly retried.

The concrete retry count is an implementation detail and may remain small and bounded.

## 24. Reliability model

### 24.1 Before external side effects

While consequential effects remain inside PostgreSQL, reliability uses:

- ACID transactions,
- foreign keys,
- UNIQUE/CHECK/partial unique indexes,
- row locks,
- idempotent application operations,
- bounded transaction retry.

Do not introduce saga/workflow infrastructure merely because Medusa has a workflow engine.

### 24.2 Breakpoint: Payment and other external systems

Payment introduces a non-transactional external boundary:

```text
Rust
 |- PostgreSQL
 `- payment provider
```

A PostgreSQL rollback cannot undo an external charge.

That is when the architecture must evaluate:

- durable payment-attempt state,
- provider idempotency keys,
- reconciliation,
- retry policy,
- compensation/refund behavior,
- outbox/background jobs,
- durable workflow execution if justified.

Medusa's workflow engine tracks execution/step state and supports compensation across multi-step operations. This project intentionally delays equivalent machinery until the corresponding failure mode exists.

References:

- https://docs.medusajs.com/learn/fundamentals/modules/isolation
- https://docs.medusajs.com/learn/fundamentals/modules/infrastructure-modules

## 25. Observability

V1 requires structured logging, not a full telemetry platform.

Use Rust `tracing`-style structured fields around application operations.

Baseline fields:

```text
request_id
actor_id when authenticated
actor_kind
channel
operation
resource identifiers such as cart_id/order_id
outcome/error code
duration
```

`request_id` means one HTTP attempt.

A future `operation_request_id` means one durable business intention and may span multiple HTTP attempts.

Application-operation boundaries are the primary business observability boundary.

Do not log secrets or raw credentials.

## 26. Health and readiness

Expose:

```text
GET /health/live
GET /health/ready
```

`live` answers whether the process is alive.

`ready` answers whether the instance can currently serve traffic, initially including PostgreSQL reachability and required configuration.

NocoDB availability must not determine storefront readiness.

## 27. Testing strategy

The most valuable tests are application-level integration tests against real PostgreSQL behavior because correctness depends on transactions, constraints, row locks, and partial indexes.

### 27.1 Module tests

Examples:

- inventory availability math,
- inventory adjustment validation,
- order state rules.

### 27.2 Application/PostgreSQL integration tests

Mandatory examples:

```text
complete_cart creates exactly one order
complete_cart snapshots item data
complete_cart reserves inventory
complete_cart marks cart completed
complete_cart is idempotent
concurrent completion cannot oversell
cancel_order releases reservation once
inventory adjustment cannot breach reserved quantity
transaction failure rolls back order/reservation/cart changes
```

### 27.3 Concurrency test

With one available unit and two customers concurrently completing carts for one unit each:

```text
exactly one completion succeeds
exactly one receives INSUFFICIENT_INVENTORY
reserved_quantity == 1
available_quantity == 0
```

### 27.4 Constraint tests

Verify actual PostgreSQL constraints for:

- one active cart per customer,
- one order per cart,
- one level per inventory item/location,
- non-negative stock and reservations,
- reserved <= stocked,
- foreign-key integrity.

### 27.5 NocoDB-role acceptance test

Verify that `commerce_nocodb` can perform approved catalog/customer edits and reads but cannot:

- update inventory quantities directly,
- mutate order state,
- insert reservations,
- alter actor security mappings,
- alter schema.

No automated NocoDB UI suite is required initially.

## 28. Backup and restore

Once real customer/order data exists, PostgreSQL backup is mandatory infrastructure.

Requirements:

- automated backups,
- documented restore procedure,
- periodic restore verification.

Backup belongs to the PostgreSQL/hosting layer, not the Rust application.

## 29. Rust repository layout

Initial layout:

```text
src/
├── main.rs
├── app.rs
├── error.rs
├── api/
│   ├── store/
│   └── ops/
├── application/
│   ├── add_cart_item.rs
│   ├── set_cart_address.rs
│   ├── adjust_inventory.rs
│   ├── complete_cart.rs
│   └── cancel_order.rs
├── catalog/
├── inventory/
├── customer/
├── cart/
├── order/
├── actor/
└── operation/        # added when required
```

A domain module initially stays shallow, for example:

```text
inventory/
├── mod.rs
├── model.rs
├── repository.rs
└── service.rs
```

Split further only when size/complexity justifies it.

### 29.1 Avoid a domain dumping ground

Do not accumulate domain concepts in a giant `common` module.

Truly cross-cutting infrastructure may live in focused modules such as:

```text
db/
auth/
http/
error/
ids/
time/
```

Domain types such as OrderStatus, InventoryReservation, or CustomerAddress remain with their owner.

### 29.2 Rust visibility

Use Rust visibility to reinforce boundaries where practical.

API code should call application operations rather than repositories directly.

```text
GOOD
api::orders -> application::cancel_order

BAD
api::orders -> inventory::repository
```

Do not split modules into separate crates until there is a concrete reason such as compile-time isolation, another binary, independent lifecycle, service extraction, or materially better context isolation.

## 30. Migration strategy

Rust owns schema migrations.

Prefer capability-oriented migration increments rather than one large initial migration, for example:

```text
001_catalog.sql
002_inventory.sql
003_actor_customer.sql
004_cart.sql
005_order.sql
006_operation_audit.sql   # only when needed
```

NocoDB never drives schema migration.

After Rust migrations change the external source, NocoDB metadata may be refreshed/synchronized as an operational step.

Current NocoDB documentation exposes data-source metadata synchronization and external-source management separately from source schema ownership.

Reference: https://nocodb.com/docs/product/integrations/data-sources

## 31. Incremental delivery plan

This is the intended architecture sequence, not yet the detailed implementation plan.

### Increment 0 - Foundation

- Rust/Axum process,
- PostgreSQL/SQLx connection,
- configuration,
- migration runner,
- error envelope,
- structured logging,
- request IDs,
- live/ready endpoints.

Success: server starts, migration runs, database readiness is observable.

### Increment 1 - Catalog

- products,
- variants,
- fixed IDR prices,
- Store product reads,
- NocoDB catalog maintenance.

Success: operators maintain catalog in NocoDB; storefront consumes it through Rust.

### Increment 2 - Inventory

- one seeded MAIN location,
- inventory items/levels/adjustments,
- availability query,
- `/ops/inventory/adjustments`,
- NocoDB inventory summary/action.

Success: useful catalog + inventory system before checkout exists.

### Increment 3 - Actor + Customer

- actors,
- customers,
- saved addresses,
- authentication adapter,
- Store `/me` and address APIs.

Success: registered customer identity exists without guest complexity.

### Increment 4 - Cart

- active cart,
- cart items,
- cart address snapshot,
- add/update/remove item,
- set shipping address.

Success: authenticated customer can prepare a checkout.

### Increment 5 - Order + Reservation

- orders/order items/order addresses,
- inventory reservations,
- `complete_cart`,
- `cancel_order`,
- locking/idempotency/concurrency tests.

Success: first complete commerce milestone.

### Increment 6 - NocoDB operational hardening

- dedicated PostgreSQL role/grants,
- read-only transactional tables,
- catalog/customer write restrictions,
- order cancellation Interface,
- inventory adjustment Interface,
- operator read views when needed.

### Increment 7 - Payment

Payment is the nearest major capability after Order.

It receives a separate design/spec because choices such as manual transfer vs provider, authorization/capture semantics, payment attempts, provider idempotency, and reconciliation materially change its architecture.

### Increment 8 - Operation audit / AI controls

- operation_requests,
- durable command provenance,
- agent principals,
- risk-based approval interception where required.

### Increment 9 - Fulfillment

- shipment/fulfillment lifecycle,
- consume reservations,
- carrier/warehouse integration where required.

### Increment 10 - POINT / mixed settlement

- wallet/account,
- immutable ledger,
- conversion policy,
- POINT-only and mixed IDR+POINT settlement.

### Increment 11 - Advanced pricing/promotions

Introduce richer Pricing/Promotion modules only when product requirements demand them.

### Increment 12 - Events / outbox / durable workflows

Introduce only when external side effects, asynchronous integrations, or operational scale justify them.

## 32. AI-assisted implementation contract

Implementation tasks should normally fit within one module or one application operation.

Each task should state:

1. architecture rule,
2. scope/module,
3. behavior/invariant,
4. completion test.

Example:

```text
Task:
Implement inventory.adjust.

Architecture:
Inventory exclusively owns inventory_levels and inventory_adjustments.

Scope:
Inventory module + application::adjust_inventory only.

Invariant:
stocked_quantity must never become lower than reserved_quantity.

Done when:
PostgreSQL integration tests cover positive adjustment,
negative adjustment, and rejection below reserved quantity.
```

Avoid broad tasks such as `build checkout` when they can be decomposed into valid intermediate repository states.

## 33. Persistent architecture documentation

Maintain a small documentation surface:

```text
docs/
├── architecture.md
├── modules/
│   ├── catalog.md
│   ├── inventory.md
│   ├── customer.md
│   ├── cart.md
│   └── order.md
└── decisions/
```

Each module document answers:

- what the module owns,
- what it exposes,
- what invariants it enforces,
- what it explicitly does not own.

This is intended to keep human and AI implementation context bounded.

## 34. Non-negotiable architecture rules

1. PostgreSQL is the source of commerce truth.
2. Rust owns schema evolution and business transitions.
3. NocoDB is an operational UI, not the commerce application layer.
4. Each module owns mutation of its tables/state.
5. Cross-module business behavior belongs in application operations.
6. One PostgreSQL transaction may span modules while the system remains a modular monolith.
7. Read models may cross module boundaries; business writes may not.
8. Transactional snapshots preserve historical truth.
9. Inventory reservation begins at order creation, not cart mutation.
10. AI agents never write PostgreSQL directly.
11. V1 restrictions are policies unless explicitly defined as domain truths.
12. Future capabilities extend orchestration boundaries rather than being absorbed into neighboring domain modules.
13. Reliability mechanisms are introduced when their corresponding failure modes exist.
14. Implementation tasks should normally fit inside one module or one application operation.
15. PostgreSQL privileges and constraints remain authoritative even if NocoDB is misconfigured.

## 35. Explicitly deferred implementation choices

The architecture does not require these choices before the first implementation plan:

- authentication provider/token format,
- concrete ID type/format,
- exact Axum router conventions,
- exact SQLx repository abstractions,
- exact retry count,
- hosting provider,
- PostgreSQL backup provider,
- NocoDB hosting mode,
- specific telemetry backend.

These choices should be made incrementally and must not violate the architecture rules above.

## 36. Known future design exercises

The following deserve separate design/spec work rather than being guessed now:

1. Payment, including external-provider failure recovery.
2. POINT wallet/ledger and mixed IDR+POINT settlement.
3. Fulfillment and shipping-provider integration.
4. Risk-based AI authorization/approval once real agent write use cases exist.
5. Rich Pricing/Promotion behavior.
6. Event/outbox/durable workflow infrastructure once external side effects require it.

## 37. Architecture-reference notes

This design is informed by current Medusa and NocoDB behavior but intentionally simplifies them.

### Medusa

- HTTP -> workflow -> module -> datastore layering:  
  https://docs.medusajs.com/learn/introduction/architecture
- Module isolation and workflow coordination:  
  https://docs.medusajs.com/learn/fundamentals/modules/isolation
- Reservation lifecycle:  
  https://docs.medusajs.com/resources/commerce-modules/inventory/reservations-lifecycle
- Inventory in order-placement/fulfillment flows:  
  https://docs.medusajs.com/resources/commerce-modules/inventory/inventory-in-flows
- Complete-cart behavior:  
  https://docs.medusajs.com/resources/storefront-development/checkout/complete-cart
- Current implementation inspected during design:  
  https://github.com/medusajs/medusa/blob/48a8812735f6630bcc12b3997b6d1d1f559cd492/packages/core/core-flows/src/cart/workflows/complete-cart.ts

### NocoDB

- External PostgreSQL data sources and schema/data edit permissions:  
  https://nocodb.com/docs/product/integrations/data-sources/connect-to-data-source
- Data-source management/meta sync:  
  https://nocodb.com/docs/product/integrations/data-sources
- Button-click workflow trigger:  
  https://nocodb.com/docs/workflows/nodes/trigger-nodes/button-clicked
- HTTP Request workflow action:  
  https://nocodb.com/docs/workflows/nodes/action-nodes/http-request

## 38. Acceptance criteria for this architecture

The architecture is successful if the project can evolve through the first order milestone while satisfying all of the following:

- Catalog and Inventory are independently understandable modules.
- NocoDB can operate the business without owning transactional invariants.
- A registered customer can create a cart and place an order.
- Cart completion is atomic, idempotent, and cannot oversell under tested concurrency.
- Historical Order data is stable when mutable Catalog/Customer data changes.
- Inventory adjustments and reservations remain distinct concepts.
- Payment can be added as a new module/orchestration participant without moving payment state into Order.
- AI agents can later call the same application boundary without gaining direct database write authority.
- Short implementation tasks can be completed without loading the entire repository context.
- Distributed-workflow machinery remains absent until an external non-transactional boundary makes it necessary.