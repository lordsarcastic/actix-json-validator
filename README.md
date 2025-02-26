# actix-json-validator

A user-friendly JSON extractor for Actix Web that automatically runs [`serde_valid`](https://crates.io/crates/serde_valid) validations on incoming JSON requests and returns **friendly and consistent error messages** in a style **inspired by [Django REST Framework (DRF)](https://www.django-rest-framework.org/)**.  

Top-level validation errors appear under `"non_field_errors"`, and nested field errors appear under their respective fields—making it straightforward to parse and display errors on forms or any client interface.

`actix-json-validator` is a lightweight extractor built around Actix’s `FromRequest` trait. It integrates `serde_valid` to validate your request structs, reducing boilerplate and ensuring consistent nested error structures reminiscent of the well-known DRF format.

---

## Features

- **Automatic Validation**  
  Let `serde_valid` handle your validation rules (e.g., `#[validate(min_length = 3)]`, `#[validate(range(min = 10))]`), and have your handlers receive pre-validated data.

- **Friendly, Nested Error Messages (DRF-Inspired)**  
  Any fields that fail validation are shown in an organized, nested structure akin to DRF. Top-level errors go under `"non_field_errors"` so that client code (and form handlers) can easily distinguish between global and field-specific issues.

- **Consistent JSON Format**  
  Provides a uniform and intuitive JSON response for errors, such as:
  ```json
  {
    "username": ["This field may not be blank."],
    "non_field_errors": ["Some global issue"]
  }
  ```
  This structure is reminiscent of Django REST Framework, popular for its clarity and ease of parsing by front-end frameworks or form validation libraries.

- **Built on Actix Web 4**  
  Uses `actix_web::FromRequest` for an idiomatic Rust approach.

- **Minimal Overhead**  
  One extractor, a small bit of error-handling logic, and you’re set!

---

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
actix-json-validator = "0.1.0"
actix-web = "4"
serde = "1"
serde_valid = "1"
serde_json = "1"
futures-util = "0.3"
thiserror = "2"
mime = "0.3"
```

Then run:

```sh
cargo build
```

You’ll be all set.

---

## Motivation

1. **Simplify Request Validation**  
   Writing validation logic for every request and building structured error responses can be repetitive. With `actix-json-validator`, you simply annotate your structs using `serde_valid`’s rich validation attributes, and the extractor automatically returns an error response if validation fails.

2. **DRF-Inspired, Easy-to-Parse Error Format**  
   Inspired by the popular **Django REST Framework** style, the library returns a uniform JSON structure with top-level errors grouped under `"non_field_errors"` and nested field errors under their respective field names. This consistency makes it much easier for client-side code or form libraries to handle and display error messages.

3. **Reduced Boilerplate**  
   No more building custom error handlers or manually scanning validation errors. This crate provides a quick integration that “just works,” so you can focus on implementing features rather than repeating error-handling patterns.

---

## Usage

### 1. Define Your Request Struct

Annotate your request structs with [`serde_valid` validation attributes](https://docs.rs/serde_valid/latest/serde_valid/):

```rust
use serde::Deserialize;
use serde_valid::Validate;

#[derive(Debug, Deserialize, Validate)]
struct CreateUser {
    #[validate(min_length = 3)]
    username: String,

    #[validate(range(min = 1, max = 120))]
    age: u8,
}
```

### 2. Use `AppJson<T>` in Your Actix Handler

```rust
use actix_web::{post, HttpResponse, Responder};
use actix_json_validator::AppJson; // from this crate

#[post("/user")]
async fn create_user(body: AppJson<CreateUser>) -> impl Responder {
    // body has already been validated!
    let user_data = body.into_inner();
    // ... do something with user_data ...

    HttpResponse::Ok().json({ "status": "success" })
}
```

### 3. Enjoy Clear Error Responses

If the user sends invalid data, the response will automatically be an HTTP 400 with a JSON body like:

```json
{
  "username": ["The length of the value must be `>= 3`."]
}
```

Or, for a top-level error:

```json
{
  "non_field_errors": ["Some global issue"]
}
```

**All structured with familiar DRF-inspired field/key groupings.**

---

## Advanced Customization

- **`JsonConfig`**: Tweak the maximum payload size or restrict content types via a predicate:
  ```rust
  JsonConfig::default()
      .limit(65536) // max payload 64 KB
      .content_type(|mime| mime.subtype() == mime::JSON)
  ```
  
- **Nested Objects & Arrays**: The crate’s internal `process_errors` method builds a nested map structure for complex fields (e.g., `{"profile": {"address": ["Cannot be empty"]}}`).

- **Customizing the Format**: Feel free to fork or copy `process_errors` if you want a different structure or to localize messages differently.

---

## Example
Example usage of the crate can be found in the [docs/examples/good-foods](./docs/examples/good-foods/) directory. To run the example, build the application with `cargo build` and run it with `cargo run`:

```sh
good-foods$ cargo build
   Compiling good-foods v0.1.0 (/actix-json-validator/docs/examples/good-foods)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.42s

good-foods$ cargo run
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/good-foods`
Starting good-foods server on http://localhost:8080
  
```

Test the application by sending a POST request to `http://localhost:8080/food` with a JSON body:

```sh
good-foods$ curl -X POST -H "Content-Type: application/json" \
  -d '{"name": "Ice Cream", "rating": 9}' \
  http://localhost:8080/foods
```

You should see a response like so:

```json
{"name":"Ice Cream","rating":9}
```

To test validation errors, send a POST request with invalid data:

```sh
good-foods$ curl -X POST -H "Content-Type: application/json" \
  -d '{"name": "Ice Cream", "rating": 12}' \
  http://localhost:8080/foods
```

You should see a response like:

```json
{"rating":["The number must be `<= 10`."]}
```
---

## Limitations

- **Actix Web 4**: This crate is designed for Actix Web 4.  
- **JSON only**: Non-JSON or incorrectly typed payloads lead to a 400 response, with an error key of `"error"` containing the error text.  
- **serde_valid**: All validations rely on `serde_valid` attributes; any custom logic must integrate at the struct level or via custom validators.

---

## Contributing

Want to improve error formatting or add features? Pull requests and issues are welcome:

1. Fork the repository.
2. Create a feature branch.
3. Write tests for your changes.
4. Submit a PR with a clear description of your improvement or fix.

---

## License

<details>
<summary><strong>MIT License</strong></summary>

```
[Your License Text Here]
```
</details>

`actix-json-validator` is distributed under the terms of the MIT license. See [LICENSE](./LICENSE) for details.

---

**Thank you for checking out `actix-json-validator`!** If you have any questions, suggestions, or issues, please open an issue on the repository. Enjoy concise, easy-to-parse validation error messages for your Actix apps!