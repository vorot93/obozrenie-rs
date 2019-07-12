use futures01::prelude::*;
use rgs::{dns::Resolver, models::TProtocol, ping::Pinger};
use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Clone)]
pub struct Querier {
    pub protocol: TProtocol,
    pub master_servers: Vec<(String, u16)>,
    pub port: u16,
    pub resolver: Arc<dyn Resolver>,
    pub pinger: Arc<dyn Pinger>,
}

impl super::Querier for Querier {
    fn query(&self) -> Box<dyn Stream<Item = rgs::models::Server, Error = failure::Error> + Send> {
        let mut query_builder = rgs::UdpQueryBuilder::default();

        query_builder = query_builder.with_pinger(self.pinger.clone());

        let socket = UdpSocket::bind(&format!("[::]:{}", self.port).parse().unwrap()).unwrap();
        let mut q = query_builder.build(socket);

        for entry in &self.master_servers {
            q.start_send(rgs::models::UserQuery {
                protocol: self.protocol.clone(),
                host: entry.clone().into(),
            })
            .unwrap();
        }

        Box::new(q.map(|e| e.data))
    }
}
