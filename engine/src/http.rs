//! HTTPS for the whole client (auth.rs, xhome.rs) behind one small interface:
//! method, URL, headers and body → status and text. Uses reqwest with the
//! Windows TLS stack (Schannel).

use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

impl Response {
    pub fn ok(&self) -> bool { (200..300).contains(&self.status) }
}

/// A client; clones share their open connections.
#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
}

impl Http {
    /// `timeout`: the maximum per request, so a reply that never comes cannot
    /// hang the start or the stop of a session.
    pub fn new(timeout: Duration) -> Result<Self> {
        Ok(Self { client: reqwest::Client::builder().timeout(timeout).build()? })
    }

    pub async fn request(&self, method: &str, url: &str, headers: &[(&str, &str)], body: Option<String>) -> Result<Response> {
        let method = reqwest::Method::from_bytes(method.as_bytes())?;
        let mut request = self.client.request(method, url);
        for (name, value) in headers { request = request.header(*name, *value) }
        if let Some(body) = body { request = request.body(body) }
        let response = request.send().await?;
        let status = response.status().as_u16();
        Ok(Response { status, body: response.text().await? })
    }
}

#[cfg(test)]
mod tests {
    use super::Response;

    #[test]
    fn ok_means_2xx() {
        assert!(Response { status: 204, body: String::new() }.ok());
        assert!(!Response { status: 401, body: String::new() }.ok());
    }
}
