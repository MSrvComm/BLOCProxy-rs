const CLIENTPORT: &str = "32655";
const PROXYINPORT: &str = "62081"; // which port will the reverse proxy use for making outgoing request
const PROXYOUTPORT: &str = "62082"; // which port the reverse proxy listens on

use std::{
    io::{self, BufReader, BufWriter},
    time::Duration,
};

use actix_web::{error, get, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use futures::future;
use reqwest::{header::HeaderMap, Client, ClientBuilder, Method, Request, Url};
use thiserror::Error;

#[derive(Clone)]
struct OutUrl {
    url: Url,
    client: Client,
}

#[derive(Error, Debug)]
enum HotMess {
    #[error("Invalid length received")]
    InvalidLength,
    #[error("Could not fetch body: {0}")]
    InvalidBody(String),
    #[error("Could not copy response body: {0}")]
    IoCopyErr(String),
    #[error("Failed to flush the buffer: {0}")]
    FailedBufFlush(String),
}

#[get("/")]
async fn get() -> impl Responder {
    format!("listening for in on {}\n", PROXYINPORT)
}

async fn get_out(req: HttpRequest, data: web::Data<OutUrl>) -> impl Responder {
    let mut headermap = HeaderMap::new();
    for (key, val) in req.headers() {
        headermap.insert(key.clone(), val.clone());
    }

    let uri = Url::parse(&format!(
        "{}{}",
        data.url.as_str(),
        &req.uri().to_string()[1..]
    ))
    .unwrap();
    println!("outgoing uri: {}", uri);

    let client = &data.client.clone();
    let request = Request::new(Method::GET, uri);

    let handle = async_std::task::spawn(client.execute(request));
    let res = match handle.await {
        Ok(res) => res,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error requesting path: {}", e))
        }
    };

    let ln = match res.content_length() {
        Some(ln) => ln as usize,
        None => return HttpResponse::from(error::ErrorInternalServerError(HotMess::InvalidLength)),
    };
    let content = match res.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return HttpResponse::from(error::ErrorInternalServerError(HotMess::InvalidBody(
                e.to_string(),
            )))
        }
    };
    let mut w = BufWriter::new(vec![0u8; ln]);
    let mut r = BufReader::new(&content[..]);
    match io::copy(&mut r, &mut w) {
        Ok(n) if n == 0 => return HttpResponse::Ok().body("End of stream"),
        Ok(_) => {}
        Err(e) => {
            return HttpResponse::from(error::ErrorInternalServerError(HotMess::IoCopyErr(
                e.to_string(),
            )))
        }
    }
    let body = match w.into_inner() {
        Ok(body) => body,
        Err(e) => {
            return HttpResponse::from(error::ErrorInternalServerError(HotMess::FailedBufFlush(
                e.to_string(),
            )))
        }
    };
    HttpResponse::Ok().body(body)
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    println!("Starting the listen server!");

    // this is a client pool
    // create only once
    let client = ClientBuilder::new()
        .http2_adaptive_window(true)
        .tcp_keepalive(Duration::new(150, 0))
        .tcp_nodelay(true) // disable Nagle
        // .connect_timeout(Duration::new(150, 0))
        .connection_verbose(true)
        .build()
        .expect("Failed here");

    let redirect_url = OutUrl {
        url: Url::parse(&format!("http://localhost:{}", CLIENTPORT)).unwrap(),
        client,
    };
    println!("Redirect URL: {}", redirect_url.url);

    let s_out = HttpServer::new(move || {
        App::new()
            .default_service(web::route().to(get_out))
            .data(redirect_url.clone())
    })
    .bind(format!("127.0.0.1:{}", PROXYOUTPORT))?
    .run();

    let s_in = HttpServer::new(|| App::new().service(get))
        .bind(format!("127.0.0.1:{}", PROXYINPORT))?
        .run();

    println!("request works");
    future::try_join(s_in, s_out).await?;
    Ok(())
}
