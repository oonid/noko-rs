import re

# 1. Fix error.rs
with open("src/error.rs", "r") as f:
    err_rs = f.read()

err_rs = err_rs.replace('err.to_string(),', '"Internal database error".to_string(),')

with open("src/error.rs", "w") as f:
    f.write(err_rs)

# 2. Fix find_main_location
with open("src/inventory/repository.rs", "r") as f:
    inv_repo = f.read()

inv_repo = inv_repo.replace(
    'SELECT id FROM inventory_locations LIMIT 1',
    "SELECT id FROM inventory_locations WHERE code = 'MAIN' LIMIT 1"
).replace('// fallback if no locations exist in test? Or maybe it errors?', '')

with open("src/inventory/repository.rs", "w") as f:
    f.write(inv_repo)

# 3. Fix SKU_ALREADY_EXISTS check
with open("src/application/create_sellable_variant.rs", "r") as f:
    app_csv = f.read()

app_csv_fix = """    .map_err(|e| {
        if let AppError::Database(db_err) = &e {
            if let Some(code) = db_err.as_database_error().and_then(|e| e.code()) {
                if code == "23505" {
                    return AppError::conflict("SKU_ALREADY_EXISTS");
                }
            }
        }
        e
    })?;"""

app_csv = re.sub(r'\.map_err\(\|e\| \{.*?\}\)\?;', app_csv_fix, app_csv, flags=re.DOTALL)

with open("src/application/create_sellable_variant.rs", "w") as f:
    f.write(app_csv)

