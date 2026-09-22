import re

with open("docs/superpowers/plans/2026-09-21-commerce-v1-implementation.md", "r") as f:
    content = f.read()

task_5_replacement = """### Task 5: Ops foundation and create_sellable_variant

**Files:**
- Create:
  - src/api/ops/mod.rs
  - src/api/ops/catalog.rs
  - src/api/ops/inventory.rs
  - src/application/create_sellable_variant.rs
  - tests/operations.rs
- Modify:
  - src/api/mod.rs
  - src/application/mod.rs
  - src/application/adjust_inventory.rs
  - src/auth.rs
  - src/catalog/repository.rs
  - src/pricing/repository.rs
  - src/inventory/repository.rs

**Interfaces:**
- Produces: `OpsCaller { actor: AuthContext }`
- Produces: `POST /ops/catalog/variants` -> `OpsCaller` -> `application::create_sellable_variant`
- Produces: `POST /ops/inventory/adjustments` -> `OpsCaller` -> `application::adjust_inventory`
- Application Operations:
  - `CreateSellableVariantInput { product_id, sku, title, amount }`
  - `AdjustInventoryRequest { inventory_item_id, location_id, delta, reason, note, initiator_external_ref }`
- Rulings:
  - **Authenticated actor_id**: Must be derived exclusively from `OpsCaller.actor.actor_id`, NEVER from the request payload.
  - **initiator_external_ref**: Untrusted provenance/audit context only.

- [ ] **Step 1: Write Ops auth tests**

Explicitly test the V1 static Bearer authentication strategy:
- correct Bearer token + configured active Actor(kind=service) → accepted
- missing Authorization → 401 JSON AppError
- wrong Bearer token → 401 JSON AppError
- configured Actor missing → authentication rejected
- configured Actor inactive → authentication rejected
- configured Actor kind != service → authentication rejected
- payload `initiator_external_ref` cannot alter authenticated `actor_id` (do not allow request `actor_id` to become authority).

- [ ] **Step 2: Implement service-principal auth**

Keep static V1 Bearer authentication. Require `Authorization: Bearer <token>` against `NOCODB_SERVICE_TOKEN`.
Resolve `NOCODB_SERVICE_ACTOR_ID` and require the Actor exists, is active, and `kind = service`. Produce `OpsCaller { actor: AuthContext }`.

*Precondition: `NOCODB_SERVICE_ACTOR_ID` refers to an already-existing active `Actor(kind=service)`. Tests may seed this trust-root Actor directly. Production bootstrap of the initial service Actor is a deployment prerequisite and is not implemented as an unauthenticated HTTP API.*

- [ ] **Step 3: Atomicity test strategy**

Write a real PostgreSQL atomicity test that causes Inventory provisioning to fail only AFTER ProductVariant and VariantPrice inserts have succeeded inside the transaction. Use a temporary/test-only CHECK constraint or trigger on the isolated test database.
Do not add production failure-injection code.
Required proof after rollback: `ProductVariant` ABSENT, `VariantPrice` ABSENT, `InventoryItem` ABSENT, `InventoryLevel` ABSENT.

- [ ] **Step 4: Implement create_sellable_variant**

Must explicitly use a single transaction. The application layer owns the transaction and orchestration; the repository modules own their respective INSERTs.
```text
BEGIN
1. validate Product exists
2. Catalog repository: create ProductVariant
3. Pricing repository: create fixed IDR VariantPrice
4. Inventory repository: create InventoryItem
5. Inventory repository: resolve active MAIN location
6. Inventory repository: create InventoryLevel (stocked_quantity = 0, reserved_quantity = 0)
7. COMMIT
```
Return `variant_id, price_id, inventory_item_id, inventory_level_id`.
Validation before DB mutation: `amount < 0` → `422 INVALID_PRICE_AMOUNT` (domain validation).
Expected conflicts: Duplicate SKU → stable domain conflict (e.g., PostgreSQL `23505` mapping) instead of generic `500 DATABASE_ERROR`.

- [ ] **Step 5: Implement Ops routes and adjust_inventory validation**

Expose `POST /ops/catalog/variants` and `POST /ops/inventory/adjustments` via `OpsCaller`. Handlers must not directly call Catalog/Pricing/Inventory mutation repositories.
Validation before entering adjustment transaction: `delta == 0` → `422 INVALID_INVENTORY_DELTA`.
Preserve: overflow → `INVENTORY_QUANTITY_OVERFLOW`, resulting stocked < 0 → `NEGATIVE_STOCK`.
Audit actor_id written to inventory_adjustments must equal `OpsCaller.actor.actor_id`.

- [ ] **Step 6: Verify**

Run tests covering: Ops auth success/failure matrix, initiator_external_ref security, `create_sellable_variant` success, negative price 422, duplicate SKU stable conflict, late Inventory failure rollback, `adjust_inventory` through Ops, `delta=0` 422, and inventory adjustment audit `actor_id` matching authenticated service Actor ID.
```bash
cargo test --test operations
cargo test --all
```
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src tests migrations
git commit -m "feat: ops service authentication and variants"
```

---

### Task 5B: Registered Customer Actor provisioning

**Files:**
- Create:
  - src/application/provision_registered_customer.rs
  - tests/customer_provisioning.rs (or keep in tests/operations.rs if explicitly cleaner)
- Modify:
  - src/application/mod.rs
  - src/actor/repository.rs
  - src/customer/repository.rs
  - src/api/ops/mod.rs
  - src/api/ops/customers.rs

**Interfaces:**
- Produces: `application::provision_registered_customer(...)`
- Input: `ProvisionRegisteredCustomerInput { auth_subject, display_name, email, phone, first_name, last_name }` (Actor.kind is always human; caller does not choose).
- Returns: `actor`, `customer` (or their IDs).
- Route: `POST /ops/customers` requiring `OpsCaller`.

- [ ] **Step 1: Implement provision_registered_customer**

Explicit sequence:
```text
BEGIN
Actor repository: create Actor(kind=human)
Customer repository: create Customer(actor_id=<new Actor>)
COMMIT
```
Application layer owns the transaction. Actor repository owns Actor INSERT. Customer repository owns Customer INSERT.

- [ ] **Step 2: Ops-only Route boundary**

Expose via `POST /ops/customers` (requires `OpsCaller`).
Do NOT expose `POST /store/customers` or any public Store provisioning API.
Exclude: passwords, OAuth, JWT issuance, email verification, roles, RBAC, sessions, address creation (which remains `POST /store/me/addresses`).

- [ ] **Step 3: Write atomic rollback test**

Use real PostgreSQL test. Existing Customer uses `email = duplicate@example.com`. Provision request uses unique `auth_subject` but `email = duplicate@example.com`.
Inside operation: Actor INSERT succeeds, Customer INSERT fails on email UNIQUE.
After rollback: Verifies no new Actor exists for `auth_subject`, no new Customer exists.

- [ ] **Step 4: Verify and Commit**

```bash
cargo test --test customer_provisioning
```
Expected: PASS.

```bash
git add src tests
git commit -m "feat: atomic customer actor provisioning"
```

---
"""

content = re.sub(r'### Task 5: Ops foundation and create_sellable_variant.*?(?=### Task 6:)', task_5_replacement, content, flags=re.DOTALL)

with open("docs/superpowers/plans/2026-09-21-commerce-v1-implementation.md", "w") as f:
    f.write(content)
