with open(".superpowers/sdd/2026-09-21-commerce-v1-implementation/progress.md", "r") as f:
    content = f.read()

task_5b_completion = """
### Task 5B Final Status
Task 5B: fix round 1/5
(all closure-evidence findings addressed, 0 open;
commits 8333109..2aed954)

Task 5B: complete
(commits ced9e8b..2aed954, review clean)
"""

if "Task 5B Final Status" not in content:
    content += task_5b_completion
    with open(".superpowers/sdd/2026-09-21-commerce-v1-implementation/progress.md", "w") as f:
        f.write(content)
