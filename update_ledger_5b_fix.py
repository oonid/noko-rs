with open(".superpowers/sdd/2026-09-21-commerce-v1-implementation/progress.md", "r") as f:
    content = f.read()

task_5b_deferred = """
### Minor (Deferred) Task 5B Structure
- **Finding:** `ProvisionRegisteredCustomerInput` currently lives at the Ops HTTP boundary (`src/api/ops/customers.rs`), while `application::provision_registered_customer` accepts explicit typed parameters rather than an application command struct.
- **Decision:** Defer refactoring. Revisit only if later application call sites make a command struct useful.
"""

if "Task 5B Structure" not in content:
    content += task_5b_deferred
    with open(".superpowers/sdd/2026-09-21-commerce-v1-implementation/progress.md", "w") as f:
        f.write(content)
