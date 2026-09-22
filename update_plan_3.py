import re

with open("docs/superpowers/plans/2026-09-21-commerce-v1-implementation.md", "r") as f:
    content = f.read()

task_6_additions = """
*Deferred Deadline Requirement: Task 6 entry requirement is to normalize relevant Axum Path/Json request rejections into the Noko JSON error envelope before Cart expands the mutation/path API surface.*

"""
content = content.replace("### Task 6: Cart schema, snapshots, address copy, and cart mutations\n", "### Task 6: Cart schema, snapshots, address copy, and cart mutations\n" + task_6_additions)

with open("docs/superpowers/plans/2026-09-21-commerce-v1-implementation.md", "w") as f:
    f.write(content)
