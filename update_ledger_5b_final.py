with open(".superpowers/sdd/2026-09-21-commerce-v1-implementation/progress.md", "r") as f:
    content = f.read()

task_5b_invariants = """
### Task 5B Verified Invariants
- **Ops boundary**: `POST /ops/customers` requires existing `OpsCaller`, no duplicate auth implementation.
- **Provisioning**: `application::provision_registered_customer` owns transaction, Actor repository owns Actor INSERT, Customer repository owns Customer INSERT, created Actor is always `kind=human`, created Actor `active=true`, caller cannot choose `actor_id`/`kind`/`active`, `Customer.actor_id` = created `Actor.id`.
- **Principal separation**: authenticated service Actor != provisioned human Actor.
- **Atomicity**: duplicate `customers.email` failure rolls back new Actor, Customer count unchanged after failed provisioning.
- **Store integration**: provisioned `auth_subject` resolves via `X-Dev-Auth-Subject`, `GET /store/me` returns exact provisioned Customer ID, returned `Customer.actor_id` equals provisioned Actor ID.
- **Scope**: no address provisioning, no public Store registration, no schema/migration changes.

### Task 5B Review Evidence
- **Task 5B independent task review**: spec compliance = PASS, code quality = APPROVED, remaining findings = NONE
- **Deep-chat remote verification**: HEAD = `2aed9541d70105a9bff7e38662bac5bdd0b03e1f`, CI run = `35722877835`, CI conclusion = success
"""

if "Task 5B Verified Invariants" not in content:
    content += task_5b_invariants
    with open(".superpowers/sdd/2026-09-21-commerce-v1-implementation/progress.md", "w") as f:
        f.write(content)
