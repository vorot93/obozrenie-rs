use failure::{Error, Fallible};
use futures::prelude::await;
use futures::prelude::*;
use librgs::{dns::Resolver, ping::Pinger, Host, Server, StringAddr};
use reqwest;
use serde_json::Value;
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
struct RigsOfRodsServer {
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

pub struct RigsOfRodsQuery {
    inner: Box<Stream<Item = Server, Error = Error> + Send>,
}

#[async_stream(item = Server)]
fn query(addr: String, dns: Arc<Resolver>, pinger: Arc<Pinger>) -> Fallible<()> {
    let mut rsp = await!(reqwest::async::Client::new().get(&format!("{}?json=true", addr)).send())?;

    let data = await!(rsp.json::<Vec<RigsOfRodsServer>>())?;

    for entry in data {
        if let Ok(addr) = await!(dns.resolve(Host::S(StringAddr {
            host: entry.ip,
            port: entry.port
        }))) {
            let ping = await!(pinger.ping(addr.ip())).unwrap_or(None);

            stream_yield!(Server {
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
                ..Server::new(addr)
            });
        }
    }

    Ok(())
}

impl RigsOfRodsQuery {
    pub fn new<S>(master_addr: S, dns: Arc<Resolver>, pinger: Arc<Pinger>) -> Self
    where
        S: ToString,
    {
        Self {
            inner: Box::new(query(master_addr.to_string(), dns, pinger)),
        }
    }
}

impl Stream for RigsOfRodsQuery {
    type Item = Server;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.inner.poll()
    }
}
