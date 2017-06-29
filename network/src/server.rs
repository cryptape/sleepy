use std::net::SocketAddr;
use std::{io, thread};
use std::sync::mpsc::Sender;

use futures::{BoxFuture, Future};
use futures::future::result;
use tokio_proto::TcpServer;
use tokio_service::Service;

use config::SleepyConfig;
use protocol::{SleepyProto, SleepyRequest, SleepyResponse};
use msghandle::net_msg_handler;

#[derive(Clone)]
pub struct MySender {
    tx: Sender<(u32, SleepyRequest)>,
}

impl MySender {
    pub fn new(tx: Sender<(u32, SleepyRequest)>) -> Self {
        MySender { tx: tx }
    }

    pub fn send(&self, msg: (u32, SleepyRequest)) {
        self.tx.send(msg).unwrap();
    }
}

unsafe impl Sync for MySender {}

struct Server {
    mysender: MySender,
}

impl Service for Server {
    type Request = SleepyRequest;
    type Response = SleepyResponse;
    type Error = io::Error;
    type Future = BoxFuture<Self::Response, io::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        result(net_msg_handler(req, &self.mysender)).boxed()
    }
}

pub fn start_server(config: &SleepyConfig, tx: Sender<(u32, SleepyRequest)>) {
    let mysender = MySender::new(tx);
    let addr = format!("0.0.0.0:{}", config.port.unwrap());
    let addr = addr.parse::<SocketAddr>().unwrap();

    thread::spawn(move || {
                      info!("start server on {:?}!", addr);
                      TcpServer::new(SleepyProto, addr).serve(move || {
                                                                Ok(Server {
                                                                       mysender: mysender.clone(),
                                                                   })
                                                            });
                  });
}
