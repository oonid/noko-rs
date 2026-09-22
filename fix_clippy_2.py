import re

with open("src/application/create_sellable_variant.rs", "r") as f:
    app_csv = f.read()

app_csv_fix = """    .map_err(|e| match &e {
        AppError::Database(db_err) if db_err.as_database_error().and_then(|err| err.code()).as_deref() == Some("23505") => {
            AppError::conflict("SKU_ALREADY_EXISTS")
        }
        _ => e,
    })?;"""

app_csv = re.sub(r'\.map_err\(\|e\| \{.*?\}\)\?;', app_csv_fix, app_csv, flags=re.DOTALL)

with open("src/application/create_sellable_variant.rs", "w") as f:
    f.write(app_csv)

