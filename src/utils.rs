use crate::{AlternateTitle, Client, PriorityQueue, Year};
use std::ops::Deref;

pub struct ScrapedFwTitleData {
    pub id: u32,
    pub year: Year,
    pub name: String,
    pub url: String,
    pub alter_titles: PriorityQueue<AlternateTitle, u8>,
    pub duration: Option<u16>, // in minutes
}

#[derive(Debug)]
pub struct ClientPool {
    clients: Vec<Client>,
}

impl ClientPool {
    pub fn new(client_sample: Client, amount: u8) -> Self {
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
