// Obozrenie Game Server Browser
// Copyright (C) 2018-2019  Artem Vorotnikov
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

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
