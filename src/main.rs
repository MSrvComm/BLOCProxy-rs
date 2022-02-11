const CLIENTPORT: &str = "32655";
const PROXYINPORT: &str = "62081"; // which port will the reverse proxy use for making outgoing request
const PROXYOUTPORT: &str = "62082"; // which port the reverse proxy listens on

mod controllercom;
mod incoming;
mod outgoing;
mod utils;

use std::{io, time::Duration};

use actix_web::{web, App, HttpServer};
use futures::future;
use reqwest::{header, ClientBuilder, Url};
use utils::{InClient, OutUrl};

use crate::{incoming::get_in, outgoing::get_out};

#[actix_web::main]
async fn main() -> io::Result<()> {
    println!("Starting the listen server!");

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

    let redirect_url = OutUrl {
        url: Url::parse(&format!("http://localhost:{}", CLIENTPORT)).unwrap(),
        out_client,
    };
    let in_c = InClient { in_client };
    println!("Redirect URL: {}", redirect_url.url);

    let s_out = HttpServer::new(move || {
        App::new()
            .default_service(web::route().to(get_out))
            .data(redirect_url.clone())
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

    println!("request works");
    future::try_join(s_in, s_out).await?;
    Ok(())
}
