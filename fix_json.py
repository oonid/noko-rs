with open("tests/operations.rs", "r") as f:
    text = f.read()

text = text.replace('body["error"]["code"]', 'body["code"]')

with open("tests/operations.rs", "w") as f:
    f.write(text)
