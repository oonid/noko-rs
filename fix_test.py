with open("tests/cart.rs", "r") as f:
    lines = f.readlines()

out = []
b_actor = False
for line in lines:
    if "actor_b" in line and "auth1" in line:
        line = line.replace("auth1", "auth_b")
        b_actor = True
    elif "actor_b" in line:
        b_actor = True
    elif b_actor and "auth1" in line:
        if "actor_a" not in line:
            line = line.replace("auth1", "auth_b")
            b_actor = False
    
    out.append(line)

with open("tests/cart.rs", "w") as f:
    f.writelines(out)
