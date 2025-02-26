use actix_json_validator::AppJson;
use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

#[derive(Debug, Deserialize, Validate, Serialize)]
struct FoodChoice {
    /// The name of the food. Must be at least 3 characters.
    #[validate(min_length = 3)]
    name: String,

    /// A rating from 1 to 10 for how much you like it.
    #[validate(minimum = 1)]
    #[validate(maximum = 10)]
    rating: u8,
}

#[post("/foods")]
async fn create_food(food_data: AppJson<FoodChoice>) -> impl Responder {
    // At this point, `food_data` is validated. If the request body is invalid,
    // `actix-json-validator` will have already returned an error response.

    let food = food_data.into_inner();

    // In a real app, you might store `food` in a database. Here, we just echo it back.
    HttpResponse::Ok().json(food)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting good-foods server on http://localhost:8080");
    HttpServer::new(|| {
        App::new()
            // Register our create_food handler
            .service(create_food)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
