import re

with open("src/application/create_sellable_variant.rs", "r") as f:
    app_csv = f.read()

app_csv_fix = """    .map_err(|e| {
        if let AppError::Database(db_err) = &e {
            if db_err.as_database_error().and_then(|e| e.code()).as_deref() == Some("23505") {
                return AppError::conflict("SKU_ALREADY_EXISTS");
            }
        }
        e
    })?;"""

app_csv = re.sub(r'\.map_err\(\|e\| \{.*?\}\)\?;', app_csv_fix, app_csv, flags=re.DOTALL)

with open("src/application/create_sellable_variant.rs", "w") as f:
    f.write(app_csv)

