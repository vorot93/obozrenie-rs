use super::*;

use failure::{Error, Fallible};
use futures::prelude::await;
use librgs::{dns::Resolver, ping::Pinger, Host, StringAddr};
use log::error;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
struct Server {
    #[serde(rename = "has-password")]
    pub has_password: u8,
    #[serde(rename = "current-users")]
    pub current_users: u8,
    #[serde(rename = "max-clients")]
    pub max_clients: u8,
    pub verified: u8,
    #[serde(rename = "is-official")]
    pub is_official: u8,
    pub ip: String,
    pub port: u16,
    #[serde(rename = "terrain-name")]
    pub terrain_name: String,
    pub name: String,
}

struct Query {
    inner: Box<dyn Stream<Item = librgs::Server, Error = Error> + Send>,
}

#[async_stream(item = librgs::Server)]
fn query(addr: String, dns: Arc<dyn Resolver>, pinger: Arc<dyn Pinger>) -> Fallible<()> {
    let mut rsp = await!(reqwest::async::Client::new()
        .get(&format!("{}?json=true", addr))
        .send())?;

    let data = await!(rsp.json::<Vec<Server>>())?;

    for entry in data {
        if let Ok(addr) = await!(dns.resolve(Host::S(StringAddr {
            host: entry.ip,
            port: entry.port
        }))) {
            let ping = await!(pinger.ping(addr.ip())).unwrap_or_else(|e| {
                error!("Failed to ping {}: {}", addr, e);
                None
            });

            stream_yield!(librgs::Server {
                ping,
                name: Some(entry.name),
                map: Some(entry.terrain_name),
                num_clients: Some(u64::from(entry.current_users)),
                max_clients: Some(u64::from(entry.max_clients)),
                rules: vec![
                    ("is_official", Value::from(entry.is_official == 1)),
                    ("verified", Value::from(entry.verified == 1))
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
                ..librgs::Server::new(addr)
            });
        }
    }

    Ok(())
}

impl Query {
    pub fn new<S>(master_addr: S, dns: Arc<dyn Resolver>, pinger: Arc<dyn Pinger>) -> Self
    where
        S: ToString,
    {
        Self {
            inner: Box::new(query(master_addr.to_string(), dns, pinger)),
        }
    }
}

impl Stream for Query {
    type Item = librgs::Server;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.inner.poll()
    }
}

#[derive(Clone)]
pub struct Querier {
    pub master_addr: String,
    pub resolver: Arc<dyn Resolver>,
    pub pinger: Arc<dyn Pinger>,
}

impl super::Querier for Querier {
    fn query(&self) -> Box<dyn Stream<Item = librgs::Server, Error = failure::Error> + Send> {
        Box::new(Query::new(
            &self.master_addr,
            self.resolver.clone(),
            self.pinger.clone(),
        ))
    }
}
