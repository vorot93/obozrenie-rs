
use failure::Error;
use futures::{compat::*, prelude::*};
use futures01::{Poll, Stream};
use gen_stream::*;
use log::error;
use reqwest::r#async::Client as HttpClient;
use rgs::{
    dns::Resolver,
    models::{Host, Server, StringAddr},
    ping::Pinger,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt::Display, sync::Arc};

#[derive(Serialize, Deserialize)]
struct ServerEntry {
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
    inner: Box<dyn Stream<Item = Server, Error = Error> + Send>,
}

impl Query {
    pub fn new<S>(master_addr: S, dns: Arc<dyn Resolver>, pinger: Arc<dyn Pinger>) -> Self
    where
        S: Display + Send + 'static,
    {
        use std::task::Poll;

        Self {
            inner: Box::new(
                Box::pin(GenTryStream::from(static move || {
                    let mut rsp = gen_await!(HttpClient::new()
                        .get(&format!("{}?json=true", master_addr))
                        .send()
                        .compat())?;

                    let data = gen_await!(rsp.json::<Vec<ServerEntry>>().compat())?;

                    for entry in data {
                        if let Ok(addr) = gen_await!(dns
                            .resolve(Host::S(StringAddr {
                                host: entry.ip,
                                port: entry.port
                            }))
                            .compat())
                        {
                            let ping =
                                gen_await!(pinger.ping(addr.ip()).compat()).unwrap_or_else(|e| {
                                    error!("Failed to ping {}: {}", addr, e);
                                    None
                                });

                            yield Poll::Ready(Server {
                                ping,
                                name: Some(entry.name),
                                map: Some(entry.terrain_name),
                                num_clients: Some(u64::from(entry.current_users)),
                                max_clients: Some(u64::from(entry.max_clients)),
                                rules: vec![
                                    ("is_official", Value::from(entry.is_official == 1)),
                                    ("verified", Value::from(entry.verified == 1)),
                                ]
                                .into_iter()
                                .map(|(k, v)| (k.to_string(), v))
                                .collect(),
                                ..Server::new(addr)
                            });
                        }
                    }

                    Ok(())
                }))
                .compat(),
            ),
        }
    }
}

impl Stream for Query {
    type Item = rgs::models::Server;
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
    fn query(&self) -> Box<dyn Stream<Item = rgs::models::Server, Error = failure::Error> + Send> {
        Box::new(Query::new(
            self.master_addr.clone(),
            self.resolver.clone(),
            self.pinger.clone(),
        ))
    }
}
