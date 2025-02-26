use std::{collections::HashMap, ops::Deref, sync::Arc};

use actix_web::{
    dev::Payload, web::JsonBody, Error, FromRequest, HttpRequest, http::StatusCode, HttpResponse, HttpResponseBuilder, ResponseError,
};
use futures_util::{future::LocalBoxFuture, FutureExt};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use serde_valid::{validation::Errors as ValidationError, Validate};


#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{{\"non_field_errors\": [\"Validation failed\"]}}")]
    ValidationError(HashMap<String, Value>),
}


impl ResponseError for AppError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            AppError::ValidationError(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let response_body = match self {
            AppError::ValidationError(errors) => {
                serde_json::json!(errors)
            }
        };

        HttpResponseBuilder::new(self.status_code()).json(response_body)
    }
}


fn format_errors(errors: ValidationError) -> HashMap<String, Value> {
    let mut result = HashMap::new();
    process_errors(&mut result, None, errors);
    result
}

fn process_errors(
    result: &mut HashMap<String, Value>,
    key: Option<String>,
    errors: ValidationError,
) {
    match errors {
        ValidationError::Array(array_errors) => {
            if !array_errors.errors.is_empty() {
                let error_messages: Vec<String> = array_errors
                    .errors
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                result.insert(
                    key.clone()
                        .unwrap_or_else(|| "non_field_errors".to_string()),
                    json!(error_messages),
                );
            }

            // Recursively process nested errors
            if !array_errors.items.is_empty() {
                let mut nested_map: HashMap<String, Value> = HashMap::new();
                for (prop, error) in array_errors.items {
                    process_errors(
                        &mut nested_map,
                        Some(prop.to_string()),
                        error,
                    );
                }
                for (prop, value) in nested_map {
                    result.insert(prop, value);
                }
            }
        }

        ValidationError::Object(object_errors) => {
            // 1) Collect any direct (top-level) errors on this object
            if !object_errors.errors.is_empty() {
                let msgs: Vec<String> = object_errors
                    .errors
                    .iter()
                    .map(ToString::to_string)
                    .collect();

                result.insert(
                    // If there's a parent key, use it; otherwise use "non_field_errors"
                    key.clone().unwrap_or_else(|| "non_field_errors".into()),
                    json!(msgs),
                );
            }

            // 2) For each property, recurse and gather its errors in a local map
            let mut child_map = serde_json::Map::new();
            for (prop, err) in object_errors.properties {
                let mut child_result = HashMap::new();
                process_errors(&mut child_result, None, err);
                // child_result is HashMap<String, Value>; we typically expect
                // it to have either "non_field_errors" or property keys.

                // Merge child_result into a single Value
                // If it has only one key that is "non_field_errors", we flatten:
                //    "prop": [ ...error array... ]
                // else store the entire map:
                //    "prop": { ... }

                if child_result.len() == 1
                    && child_result.contains_key("non_field_errors")
                {
                    child_map.insert(
                        prop,
                        child_result.remove("non_field_errors").unwrap(),
                    );
                } else {
                    child_map.insert(prop, json!(child_result));
                }
            }

            // 3) Now we have a map of child properties. If there's a parent key,
            //    nest them under that parent key. Otherwise, store them top-level.
            if !child_map.is_empty() {
                if let Some(parent) = key {
                    // If the parent key already exists in result and is an object,
                    // we can merge. If it's an array, or doesn't exist yet, handle accordingly.
                    match result.get_mut(&parent) {
                        Some(val) if val.is_object() => {
                            // Merge child_map into the existing object
                            if let Some(obj) = val.as_object_mut() {
                                for (child_prop, child_val) in child_map {
                                    obj.insert(child_prop, child_val);
                                }
                            }
                        }
                        _ => {
                            // Overwrite or create new
                            result.insert(parent, json!(child_map));
                        }
                    }
                } else {
                    // We are top-level
                    for (child_prop, child_val) in child_map {
                        result.insert(child_prop, child_val);
                    }
                }
            }
        }

        ValidationError::NewType(vec_errors) => {
            if !vec_errors.is_empty() {
                let error_messages: Vec<String> =
                    vec_errors.iter().map(ToString::to_string).collect();
                result.insert(
                    key.unwrap_or_else(|| "non_field_errors".to_string()),
                    json!(error_messages),
                );
            }
        }
    }
}

#[derive(Debug)]
pub struct AppJson<T>(pub T);

impl<T> AppJson<T> {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> AsRef<T> for AppJson<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> Deref for AppJson<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> FromRequest for AppJson<T>
where
    T: DeserializeOwned + Validate + 'static,
{
    type Error = AppError;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    #[inline]
    fn from_request(
        req: &HttpRequest,
        payload: &mut Payload,
    ) -> Self::Future {
        let (limit, ctype) = req
            .app_data::<JsonConfig>()
            .map(|c| (c.limit, c.content_type.clone()))
            .unwrap_or((32768, None));

        JsonBody::<T>::new(req, payload, ctype.as_deref(), false)
            .limit(limit)
            .map(|res| match res {
                Ok(data) => data
                    .validate()
                    .map_err(|err: serde_valid::validation::Errors| {
                        println!("{:?}", err);
                        Self::Error::ValidationError(format_errors(err))
                    })
                    .map(|_| AppJson(data)),
                Err(e) => Err(Self::Error::ValidationError({
                    let mut formatted_errors = HashMap::new();
                    formatted_errors.insert(
                        "error".to_string(),
                        json!(vec![e.to_string()]),
                    );
                    formatted_errors
                })),
            })
            .boxed_local()
    }
}

type ErrHandler =
    Arc<dyn Fn(Error, &HttpRequest) -> actix_web::Error + Send + Sync>;

#[derive(Clone)]
pub struct JsonConfig {
    limit: usize,
    ehandler: Option<ErrHandler>,
    content_type: Option<Arc<dyn Fn(mime::Mime) -> bool + Send + Sync>>,
}

impl JsonConfig {
    /// Change max size of payload. By default max size is 32Kb
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set custom error handler
    pub fn error_handler<F>(mut self, f: F) -> Self
    where
        F: Fn(Error, &HttpRequest) -> actix_web::Error
            + Send
            + Sync
            + 'static,
    {
        self.ehandler = Some(Arc::new(f));
        self
    }

    /// Set predicate for allowed content types
    pub fn content_type<F>(mut self, predicate: F) -> Self
    where
        F: Fn(mime::Mime) -> bool + Send + Sync + 'static,
    {
        self.content_type = Some(Arc::new(predicate));
        self
    }
}

impl Default for JsonConfig {
    fn default() -> Self {
        JsonConfig {
            limit: 32768,
            ehandler: None,
            content_type: None,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::body::MessageBody;
    use actix_web::http::StatusCode;
    use actix_web::web::Bytes;
    use actix_web::{test, ResponseError};
    use serde::Deserialize;
    use serde_json::json;
    use serde_valid::{validation::Error as SVError, Validate};

    #[actix_web::test]
    async fn test_field_level_error() {
        #[derive(Debug, Deserialize, Validate)]
        struct Test {
            #[validate(min_length = 3)]
            name: String,
        }
        let (req, mut payload) = test::TestRequest::post()
            .set_payload(json!({"name": "tt"}).to_string())
            .to_http_parts();

        let res = AppJson::<Test>::from_request(&req, &mut payload)
            .await
            .unwrap_err();

        assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
        let body = res.error_response().into_body().try_into_bytes().unwrap();
        assert_eq!(
            body,
            Bytes::from_static(
                b"{\"name\":[\"The length of the value must be `>= 3`.\"]}"
            )
        );
    }

    #[actix_web::test]
    async fn test_nested_field_level_error() {
        #[derive(Debug, Deserialize, Validate)]
        struct Test {
            #[validate]
            inner: Inner,
        }

        #[derive(Debug, Deserialize, Validate)]
        struct Inner {
            #[validate(min_length = 3)]
            name: String,
        }

        let (req, mut payload) = test::TestRequest::post()
            .set_payload(json!({"inner": {"name": "tt"}}).to_string())
            .to_http_parts();

        let res = AppJson::<Test>::from_request(&req, &mut payload)
            .await
            .unwrap_err();

        assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
        let body = res.error_response().into_body().try_into_bytes().unwrap();
        assert_eq!(
            body,
            Bytes::from_static(b"{\"inner\":{\"name\":[\"The length of the value must be `>= 3`.\"]}}")
        );
    }

    #[actix_web::test]
    async fn test_top_level_error() {
        /// This struct itself is "invalid" if `is_valid` is false
        /// We'll simulate a custom validator using `#[validate(schema(function = "..."))]`
        #[derive(Debug, Deserialize, Validate)]
        #[validate(custom = top_level_check)]
        struct TestStruct {
            pub data: String,
            pub is_valid: bool,
        }

        fn top_level_check(value: &TestStruct) -> Result<(), SVError> {
            if !value.is_valid || !value.data.is_empty() {
                return Err(SVError::Custom(
                    "Overall data is invalid!".to_string(),
                ));
            }
            Ok(())
        }

        // Provide invalid input so top-level fails
        let payload_data =
            json!({"data": "some stuff", "is_valid": false}).to_string();
        let (req, mut payload) = test::TestRequest::post()
            .set_payload(payload_data)
            .to_http_parts();

        let res = AppJson::<TestStruct>::from_request(&req, &mut payload)
            .await
            .unwrap_err();

        // We expect a top-level error => "non_field_errors"
        assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
        let body = res.error_response().into_body().try_into_bytes().unwrap();
        let expected_json = json!({
            "non_field_errors": ["Overall data is invalid!"]
        });
        let expected_string = expected_json.to_string(); // keep this string in a variable
        let expected_bytes = Bytes::from(expected_string); // create Bytes from that string

        assert_eq!(body, expected_bytes);
    }

    /// 2) Test array-level validation error
    #[actix_web::test]
    async fn test_array_error() {
        /// Suppose each item in `items` must be >= 3 chars
        #[derive(Debug, Deserialize, Validate)]
        struct ArrayStruct {
            #[validate(min_items = 2)] // at least 2 items
            items: Vec<String>,
        }

        // Provide invalid data: only 1 item, length < 3
        let payload_data = json!({"items": ["ab"]}).to_string();
        let (req, mut payload) = test::TestRequest::post()
            .set_payload(payload_data)
            .to_http_parts();

        let res = AppJson::<ArrayStruct>::from_request(&req, &mut payload)
            .await
            .unwrap_err();

        assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
        let body = res.error_response().into_body().try_into_bytes().unwrap();

        let expected = json!({
            "items": ["The length of the items must be `>= 2`."]
        });
        let expected_string = expected.to_string();
        let expected_bytes = Bytes::from(expected_string);
        assert_eq!(body, expected_bytes);
    }

    /// 3) Test multiple nested properties failing
    #[actix_web::test]
    async fn test_multiple_nested_errors() {
        #[derive(Debug, Deserialize, Validate)]
        struct Parent {
            #[validate]
            inner1: Inner,
            #[validate]
            inner2: Inner,
        }

        #[derive(Debug, Deserialize, Validate)]
        struct Inner {
            #[validate(min_length = 3)]
            name: String,
            #[validate(minimum = 10)]
            age: u8,
        }

        let payload_data = json!({
            "inner1": {"name": "ab", "age": 9},
            "inner2": {"name": "cd", "age": 5}
        })
        .to_string();
        let (req, mut payload) = test::TestRequest::post()
            .set_payload(payload_data)
            .to_http_parts();

        let res = AppJson::<Parent>::from_request(&req, &mut payload)
            .await
            .unwrap_err();

        assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
        let body = res.error_response().into_body().try_into_bytes().unwrap();

        let expected = json!({
            "inner1": {
                "name": ["The length of the value must be `>= 3`."],
                "age": ["The number must be `>= 10`."]
            },
            "inner2": {
                "name": ["The length of the value must be `>= 3`."],
                "age": ["The number must be `>= 10`."]
            }
        });

        let expected_string = expected.to_string();
        let expected_bytes = Bytes::from(expected_string);
        assert_eq!(body, expected_bytes);
    }

    #[actix_web::test]
    async fn test_newtype_validation_error() {
        #[derive(Debug, Deserialize, Validate)]
        struct NewTypeWrapper(#[validate(minimum = 10)] i32);

        let payload_data = json!(5).to_string(); // invalid: must be >= 10
        let (req, mut payload) = test::TestRequest::post()
            .set_payload(payload_data)
            .to_http_parts();

        let res = AppJson::<NewTypeWrapper>::from_request(&req, &mut payload)
            .await
            .unwrap_err();

        assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
        let body = res.error_response().into_body().try_into_bytes().unwrap();
        let expected = json!({
            "non_field_errors": ["The number must be `>= 10`."]
        });

        let expected_string = expected.to_string();
        let expected_bytes = Bytes::from(expected_string);
        assert_eq!(body, expected_bytes);
    }
}
