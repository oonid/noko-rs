with open(".superpowers/sdd/2026-09-21-commerce-v1-implementation/progress.md", "r") as f:
    content = f.read()

task_5b_preflight = """
### Ruling: Task 5B File Classification
- **Decision:** Task 5B treats `src/api/ops/customers.rs` as Create, not Modify.
- **Reason:** Task 5 created only `ops/mod.rs`, `ops/catalog.rs`, and `ops/inventory.rs`. The Task 5B file-list label is a bookkeeping typo.
- **Cost if wrong:** None architecturally; this only corrects the file operation classification.
"""

if "Task 5B File Classification" not in content:
    content += task_5b_preflight
    with open(".superpowers/sdd/2026-09-21-commerce-v1-implementation/progress.md", "w") as f:
        f.write(content)
