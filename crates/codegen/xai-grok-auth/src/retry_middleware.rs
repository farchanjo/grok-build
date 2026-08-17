//! `reqwest-middleware` layer: stamps auth headers and retries on 401.
//! Gated behind the `middleware` cargo feature.

use std::sync::Arc;

use reqwest::{Request, Response, StatusCode, header::HeaderValue};
use reqwest_middleware::{Error, Middleware, Next};

use crate::{AuthCredentialProvider, bearer_tail};

/// Tail fragment of the bearer used for the response's HTTP attempt.
///
/// The middleware writes this directly to each [`Response`] extension map. A
/// retry therefore cannot leave the first attempt's bearer attached to the
/// final response. Absence means the middleware did not stamp a credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StampedBearerTail(pub String);

pub struct AuthRetryMiddleware {
    credentials: Arc<dyn AuthCredentialProvider>,
    max_retries: u32,
}

impl AuthRetryMiddleware {
    pub fn new(credentials: Arc<dyn AuthCredentialProvider>, max_retries: u32) -> Self {
        Self {
            credentials,
            max_retries,
        }
    }
}

fn apply_auth_snapshot(req: &mut Request, token: Option<&str>) -> Option<StampedBearerTail> {
    req.headers_mut().remove(reqwest::header::AUTHORIZATION);
    let token = token?;
    match HeaderValue::from_str(&format!("Bearer {token}")) {
        Ok(value) => {
            req.headers_mut()
                .insert(reqwest::header::AUTHORIZATION, value);
            Some(StampedBearerTail(bearer_tail(token).to_owned()))
        }
        Err(error) => {
            tracing::warn!(error = %error, "auth retry: failed to build Authorization header");
            None
        }
    }
}

fn stamp_response(mut response: Response, stamp: Option<StampedBearerTail>) -> Response {
    response.extensions_mut().remove::<StampedBearerTail>();
    if let Some(stamp) = stamp {
        response.extensions_mut().insert(stamp);
    }
    response
}

#[async_trait::async_trait]
impl Middleware for AuthRetryMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut http::Extensions,
        next: Next<'_>,
    ) -> Result<Response, Error> {
        let initial_snapshot = self.credentials.snapshot();
        let initial_stamp = apply_auth_snapshot(&mut req, initial_snapshot.token.as_deref());

        let backup = req.try_clone();
        let response = stamp_response(next.clone().run(req, extensions).await?, initial_stamp);

        if response.status() != StatusCode::UNAUTHORIZED || self.max_retries == 0 {
            return Ok(response);
        }
        let Some(backup) = backup else {
            return Ok(response);
        };

        let mut last_response = response;
        for _ in 0..self.max_retries {
            if !self.credentials.refresh_after_unauthorized().await {
                break;
            }
            let Some(token) = self.credentials.snapshot().token else {
                break;
            };
            let Some(mut retry) = backup.try_clone() else {
                break;
            };
            let Some(retry_stamp) = apply_auth_snapshot(&mut retry, Some(&token)) else {
                break;
            };
            last_response = stamp_response(
                next.clone().run(retry, extensions).await?,
                Some(retry_stamp),
            );
            if last_response.status() != StatusCode::UNAUTHORIZED {
                return Ok(last_response);
            }
        }

        Ok(last_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CredentialSnapshot, HttpAuth};
    use reqwest_middleware::ClientBuilder;
    use std::sync::Mutex;

    struct MockProvider {
        token: Mutex<Option<String>>,
        refresh_result: bool,
        refresh_count: Mutex<u32>,
    }

    impl MockProvider {
        fn new(token: Option<&str>, refresh_result: bool) -> Self {
            Self {
                token: Mutex::new(token.map(str::to_owned)),
                refresh_result,
                refresh_count: Mutex::new(0),
            }
        }

        fn refresh_count(&self) -> u32 {
            *self.refresh_count.lock().unwrap()
        }
    }

    impl HttpAuth for MockProvider {
        fn apply(&self, builder: reqwest::RequestBuilder, _: &str) -> reqwest::RequestBuilder {
            builder
        }
    }

    #[async_trait::async_trait]
    impl AuthCredentialProvider for MockProvider {
        fn snapshot(&self) -> CredentialSnapshot {
            CredentialSnapshot {
                token: self.token.lock().unwrap().clone(),
                ..Default::default()
            }
        }

        async fn refresh_after_unauthorized(&self) -> bool {
            *self.refresh_count.lock().unwrap() += 1;
            self.refresh_result
        }
    }

    fn build_client(
        provider: Arc<dyn AuthCredentialProvider>,
        max_retries: u32,
    ) -> reqwest_middleware::ClientWithMiddleware {
        ClientBuilder::new(reqwest::Client::new())
            .with(AuthRetryMiddleware::new(provider, max_retries))
            .build()
    }

    fn response_stamp(response: &Response) -> Option<&str> {
        response
            .extensions()
            .get::<StampedBearerTail>()
            .map(|stamp| stamp.0.as_str())
    }

    #[tokio::test]
    async fn unauthorized_without_refresh_keeps_attempt_stamp() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(401)
            .expect(1)
            .create_async()
            .await;

        let provider = Arc::new(MockProvider::new(Some("token-stale-tail"), false));
        let client = build_client(provider.clone(), 1);

        let response = client.get(server.url()).send().await.unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(response_stamp(&response), Some("n-stale-tail"));
        assert_eq!(provider.refresh_count(), 1);
        mock.assert_async().await;
    }

    struct SimulatedAuthManager {
        token: Mutex<Option<String>>,
        fresh_tokens: Mutex<std::collections::VecDeque<Option<String>>>,
        refresh_count: Mutex<u32>,
    }

    impl SimulatedAuthManager {
        fn new(stale: &str, fresh: Option<&str>) -> Self {
            Self::with_refreshes(stale, [fresh])
        }

        fn with_refreshes<'a>(
            stale: &str,
            fresh: impl IntoIterator<Item = Option<&'a str>>,
        ) -> Self {
            Self {
                token: Mutex::new(Some(stale.to_owned())),
                fresh_tokens: Mutex::new(
                    fresh
                        .into_iter()
                        .map(|token| token.map(str::to_owned))
                        .collect(),
                ),
                refresh_count: Mutex::new(0),
            }
        }
    }

    impl HttpAuth for SimulatedAuthManager {
        fn apply(&self, builder: reqwest::RequestBuilder, _: &str) -> reqwest::RequestBuilder {
            builder
        }
    }

    #[async_trait::async_trait]
    impl AuthCredentialProvider for SimulatedAuthManager {
        fn snapshot(&self) -> CredentialSnapshot {
            CredentialSnapshot {
                token: self.token.lock().unwrap().clone(),
                ..Default::default()
            }
        }

        async fn refresh_after_unauthorized(&self) -> bool {
            *self.refresh_count.lock().unwrap() += 1;
            let Some(next) = self.fresh_tokens.lock().unwrap().pop_front() else {
                return false;
            };
            *self.token.lock().unwrap() = next;
            true
        }
    }

    #[tokio::test]
    async fn retry_success_carries_fresh_attempt_stamp() {
        let mut server = mockito::Server::new_async().await;
        let stale = server
            .mock("GET", "/api")
            .match_header("authorization", "Bearer token-stale-tail")
            .with_status(401)
            .create_async()
            .await;
        let fresh = server
            .mock("GET", "/api")
            .match_header("authorization", "Bearer token-fresh-tail")
            .with_status(200)
            .create_async()
            .await;

        let provider = Arc::new(SimulatedAuthManager::new(
            "token-stale-tail",
            Some("token-fresh-tail"),
        ));
        let client = build_client(provider.clone(), 1);

        let response = client
            .get(format!("{}/api", server.url()))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response_stamp(&response), Some("n-fresh-tail"));
        assert_eq!(*provider.refresh_count.lock().unwrap(), 1);
        stale.assert_async().await;
        fresh.assert_async().await;
    }

    #[tokio::test]
    async fn final_retry_unauthorized_carries_final_attempt_stamp() {
        let mut server = mockito::Server::new_async().await;
        let stale = server
            .mock("GET", "/api")
            .match_header("authorization", "Bearer token-stale-tail")
            .with_status(401)
            .create_async()
            .await;
        let fresh = server
            .mock("GET", "/api")
            .match_header("authorization", "Bearer token-fresh-tail")
            .with_status(401)
            .create_async()
            .await;

        let provider = Arc::new(SimulatedAuthManager::new(
            "token-stale-tail",
            Some("token-fresh-tail"),
        ));
        let client = build_client(provider, 1);

        let response = client
            .get(format!("{}/api", server.url()))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(response_stamp(&response), Some("n-fresh-tail"));
        stale.assert_async().await;
        fresh.assert_async().await;
    }

    #[tokio::test]
    async fn later_retry_overwrites_earlier_retry_stamp() {
        let mut server = mockito::Server::new_async().await;
        let stale = server
            .mock("GET", "/api")
            .match_header("authorization", "Bearer token-stale-tail")
            .with_status(401)
            .create_async()
            .await;
        let fresh_one = server
            .mock("GET", "/api")
            .match_header("authorization", "Bearer token-fresh-one")
            .with_status(401)
            .create_async()
            .await;
        let fresh_two = server
            .mock("GET", "/api")
            .match_header("authorization", "Bearer token-fresh-two")
            .with_status(401)
            .create_async()
            .await;

        let provider = Arc::new(SimulatedAuthManager::with_refreshes(
            "token-stale-tail",
            [Some("token-fresh-one"), Some("token-fresh-two")],
        ));
        let client = build_client(provider, 2);

        let response = client
            .get(format!("{}/api", server.url()))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(response_stamp(&response), Some("en-fresh-two"));
        stale.assert_async().await;
        fresh_one.assert_async().await;
        fresh_two.assert_async().await;
    }

    #[tokio::test]
    async fn missing_retry_credential_does_not_replace_first_attempt_response() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .match_header("authorization", "Bearer token-stale-tail")
            .with_status(401)
            .expect(1)
            .create_async()
            .await;

        let provider = Arc::new(SimulatedAuthManager::new("token-stale-tail", None));
        let client = build_client(provider, 1);

        let response = client.get(server.url()).send().await.unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(response_stamp(&response), Some("n-stale-tail"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn invalid_refreshed_token_skips_retry_and_keeps_first_attempt_stamp() {
        for invalid_token in ["token\rstale-injection", "token\nstale-injection"] {
            let mut server = mockito::Server::new_async().await;
            let mock = server
                .mock("GET", "/")
                .match_header("authorization", "Bearer token-stale-tail")
                .with_status(401)
                .expect(1)
                .create_async()
                .await;

            let provider = Arc::new(SimulatedAuthManager::new(
                "token-stale-tail",
                Some(invalid_token),
            ));
            let client = build_client(provider.clone(), 1);

            let response = client.get(server.url()).send().await.unwrap();
            assert_eq!(response.status(), 401);
            assert_eq!(response_stamp(&response), Some("n-stale-tail"));
            assert_eq!(*provider.refresh_count.lock().unwrap(), 1);
            mock.assert_async().await;
        }
    }

    #[tokio::test]
    async fn no_credential_means_no_response_stamp() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(401)
            .create_async()
            .await;

        let provider = Arc::new(MockProvider::new(None, false));
        let client = build_client(provider, 0);

        let response = client.get(server.url()).send().await.unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(response_stamp(&response), None);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn invalid_snapshot_token_clears_preexisting_authorization_header() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .match_header("authorization", mockito::Matcher::Missing)
            .with_status(200)
            .expect(1)
            .create_async()
            .await;

        let provider = Arc::new(MockProvider::new(Some("invalid\r\ntoken"), false));
        let client = build_client(provider, 0);

        let response = client
            .get(server.url())
            .header(reqwest::header::AUTHORIZATION, "Bearer stale-token")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response_stamp(&response), None);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn stamps_auth_header_automatically() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api")
            .match_header("authorization", "Bearer my-token")
            .with_status(200)
            .create_async()
            .await;

        let provider = Arc::new(MockProvider::new(Some("my-token"), false));
        let client = build_client(provider.clone(), 1);

        let response = client
            .get(format!("{}/api", server.url()))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response_stamp(&response), Some("my-token"));
        assert_eq!(provider.refresh_count(), 0);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn max_retries_bounds_attempts() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(401)
            .expect(4)
            .create_async()
            .await;

        let provider = Arc::new(MockProvider::new(Some("token"), true));
        let client = build_client(provider.clone(), 3);

        let response = client.get(server.url()).send().await.unwrap();
        assert_eq!(response.status(), 401);
        assert_eq!(response_stamp(&response), Some("token"));
        assert_eq!(provider.refresh_count(), 3);
        mock.assert_async().await;
    }
}
