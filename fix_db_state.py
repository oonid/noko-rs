with open("tests/operations.rs", "r") as f:
    text = f.read()

# Replace ON CONFLICT (code) DO NOTHING with ON CONFLICT (code) DO UPDATE SET active = true for inventory_locations
text = text.replace("INSERT INTO inventory_locations (id, code, name, active) VALUES ($1, 'MAIN', $2, true) ON CONFLICT (code) DO NOTHING", 
                    "INSERT INTO inventory_locations (id, code, name, active) VALUES ($1, 'MAIN', $2, true) ON CONFLICT (code) DO UPDATE SET active = true")

text = text.replace("INSERT INTO inventory_locations (id, code, name, active) VALUES ($1, 'MAIN', 'Main', true) ON CONFLICT (code) DO NOTHING", 
                    "INSERT INTO inventory_locations (id, code, name, active) VALUES ($1, 'MAIN', 'Main', true) ON CONFLICT (code) DO UPDATE SET active = true")

with open("tests/operations.rs", "w") as f:
    f.write(text)
