import os
import glob

for path in glob.glob("src/application/*.rs"):
    with open(path, "r") as f:
        content = f.read()

    content = content.replace('AppError::BadRequest("Quantity must be positive".to_string())', 'AppError::bad_request("invalid_quantity", "Quantity must be positive")')
    content = content.replace('AppError::NotFound("Cart not found or not active".to_string())', 'AppError::not_found("cart_not_found")')
    content = content.replace('AppError::NotFound("Variant not found, not active, or no IDR pricing".to_string())', 'AppError::not_found("variant_not_found")')
    content = content.replace('AppError::NotFound("Item not found in cart".to_string())', 'AppError::not_found("item_not_found")')
    content = content.replace('AppError::NotFound("Customer address not found".to_string())', 'AppError::not_found("address_not_found")')

    with open(path, "w") as f:
        f.write(content)
