with open(".superpowers/sdd/2026-09-21-commerce-v1-implementation/progress.md", "r") as f:
    content = f.read()

task_6_rulings = """
### Ruling: Task 6 Axum Rejection Normalization
- **Decision:** The deferred Axum Path/Json rejection requirement is Task 6 Step 0, not a separate task or separate architectural increment.
- **Reason:** It is specifically an entry condition for the new Cart HTTP surface, and is small enough to implement and verify before the Cart routes inside the same Task 6 cycle.
- **Cost if wrong:** A small extraction/error-boundary refactor would need to be separated later; no Cart domain or schema behavior depends on that separation.

### Ruling: Task 6 Missing Update/Remove Application Files
- **Decision:** Create dedicated application operations for `update_cart_item` and `remove_cart_item` rather than placing cross-layer business behavior directly in the HTTP handler.
- **Reason:** The architecture spec explicitly names these application operations, and Cart mutation locking belongs below the HTTP layer.
- **Cost if wrong:** Two small application files could later be consolidated, but the resulting architecture remains compliant and easier to review.
"""

if "Task 6 Axum Rejection Normalization" not in content:
    content += task_6_rulings
    with open(".superpowers/sdd/2026-09-21-commerce-v1-implementation/progress.md", "w") as f:
        f.write(content)
