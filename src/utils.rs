use crate::USER_AGENT;
use reqwest::blocking::Client;
use reqwest::header;
use std::ops::Deref;

#[derive(Debug)]
pub struct ClientPool {
    clients: Vec<Client>,
}

impl ClientPool {
    pub fn new(client_sample: Client, amount: u8) -> Self {
        log::debug!("Creating a ClientPool");
        let mut clients = Vec::new();
        for _ in 0..amount - 1 {
            clients.push(client_sample.clone());
        }
        clients.push(client_sample);

        Self { clients }
    }
}

impl Deref for ClientPool {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        let i = fastrand::usize(..self.clients.len());
        &self.clients[i]
    }
}

/// Creates a Reqwest HTTP client with additional headers
pub fn create_client() -> Result<Client, reqwest::Error> {
    log::debug!("Creating a Client");
    let mut headers = header::HeaderMap::new();

    headers.insert(
        header::CONNECTION,
        header::HeaderValue::from_static("keep-alive"),
    );

    headers.insert(
        header::ACCEPT_ENCODING,
        header::HeaderValue::from_static("gzip"),
    );

    // Filmweb requires this
    headers.insert(
        header::HeaderName::from_static("x-locale"),
        header::HeaderValue::from_static("pl_PL"),
    );

    Client::builder()
        .user_agent(USER_AGENT)
        .gzip(true)
        .default_headers(headers)
        .cookie_store(true)
        .build()
}
