use actix_json_validator::AppJson;
use actix_web::{post, App, HttpResponse, HttpServer, Responder};
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


#[cfg(test)]
mod tests {
    use super::*; // Pull in create_food() and FoodChoice from main.rs
    use actix_web::{body::MessageBody, http::StatusCode, test, App};
    use serde_json::json;
    
    #[actix_web::test]
    async fn test_valid_food() {
        // Arrange
        let app = test::init_service(
            App::new().service(create_food)
        ).await;

        // Act: send a valid payload
        let req = test::TestRequest::post()
            .uri("/foods")
            .set_json(&json!({
                "name": "Pizza",
                "rating": 10
            }))
            .to_request();
        
        let resp = test::call_service(&app, req).await;

        // Assert: must be 200 OK
        assert_eq!(resp.status(), StatusCode::OK);

        // Check the returned JSON body
        let body_bytes = test::read_body(resp).await;
        let returned_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(returned_json["name"], "Pizza");
        assert_eq!(returned_json["rating"], 10);
    }

    #[actix_web::test]
    async fn test_invalid_name() {
        let app = test::init_service(
            App::new().service(create_food)
        ).await;

        // Name is only 2 chars => invalid
        let req = test::TestRequest::post()
            .uri("/foods")
            .set_json(&json!({
                "name": "Ab",
                "rating": 5
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Check the returned error JSON
        let body_bytes = test::read_body(resp).await;
        let error_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        // Expect something like:
        // {
        //    "name": ["The length of the value must be `>= 3`."]
        // }
        assert!(error_json.get("name").is_some());
    }

    #[actix_web::test]
    async fn test_invalid_rating() {
        let app = test::init_service(
            App::new().service(create_food)
        ).await;

        // Rating = 0 => invalid
        let req = test::TestRequest::post()
            .uri("/foods")
            .set_json(&json!({
                "name": "Sandwich",
                "rating": 0
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body_bytes = test::read_body(resp).await;
        let error_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        // Expect something like:
        // {
        //   "rating": ["The number must be `>= 1` and `<= 10`."]
        // }
        assert!(error_json.get("rating").is_some());
    }
}