const CLIENTPORT: &str = "31104";
const PROXYINPORT: &str = "62081"; // which port will the reverse proxy use for making outgoing request
const PROXYOUTPORT: &str = "62082"; // which port the reverse proxy listens on

mod controllercom;
mod incoming;
mod mgr;
mod outgoing;
mod utils;
mod loadbalancer;

use std::{io, sync::Arc, time::Duration};

use actix_web::{web, App, HttpServer};
use async_std::channel::bounded;
use futures::future;
use reqwest::{header, ClientBuilder, Url};
use utils::{InClient, OutData};

use crate::{incoming::get_in, mgr::Mgr, outgoing::get_out};

#[actix_web::main]
async fn main() -> io::Result<()> {
    println!("Starting the listen server!");

    // start the manager
    let (snd_2_mgr, rcv_frm_actor) = bounded(40); // 40 = num_cpus
    let mgr = Arc::new(Mgr::new(rcv_frm_actor));
    let th_mgr = mgr.clone();
    let handle = std::thread::spawn(move || {
        th_mgr.clone().run();
    });

    // this is a client pool
    // create only once
    let out_client = ClientBuilder::new()
        .http2_adaptive_window(true)
        .tcp_keepalive(Duration::new(150, 0))
        .tcp_nodelay(true) // disable Nagle
        // .connect_timeout(Duration::new(150, 0))
        .connection_verbose(true)
        .build()
        .expect("Failed creating out client pool");

    let mut headers = header::HeaderMap::new();
    headers.insert("X-Forwarded-For", header::HeaderValue::from_static(""));
    let in_client = ClientBuilder::new()
        .http2_adaptive_window(true)
        .tcp_keepalive(Duration::new(150, 0))
        .tcp_nodelay(true) // disable Nagle
        .connection_verbose(true)
        .default_headers(headers)
        .build()
        .expect("Failed creating in client pool");

    let out_data = OutData {
        url: Url::parse(&format!("http://localhost:{}", CLIENTPORT)).unwrap(),
        out_client,
        mgr: mgr.clone(),
        sender: snd_2_mgr,
    };
    let in_c = InClient { in_client };
    println!("Redirect URL: {}", out_data.url);

    let s_out = HttpServer::new(move || {
        App::new()
            .default_service(web::route().to(get_out))
            .data(out_data.clone())
    })
    .bind(format!("127.0.0.1:{}", PROXYOUTPORT))?
    .run();

    let s_in = HttpServer::new(move || {
        App::new()
            .default_service(web::route().to(get_in))
            .data(in_c.clone())
    })
    .bind(format!("127.0.0.1:{}", PROXYINPORT))?
    .run();

    handle.join();
    println!("request works");
    future::try_join(s_in, s_out).await?;
    Ok(())
}
