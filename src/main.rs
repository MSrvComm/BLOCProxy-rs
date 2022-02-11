const CLIENTPORT: &str = "5000";
const PROXYINPORT: &str = "62081"; // which port will the reverse proxy use for making outgoing request
const PROXYOUTPORT: &str = "62082"; // which port the reverse proxy listens on

use std::{
    io::{self, BufReader, BufWriter},
    time::Duration,
};

use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use futures::future;
use reqwest::{header::HeaderMap, Client, ClientBuilder, Method, Request, Url};

#[derive(Clone)]
struct OutUrl {
    url: Url,
    client: Client,
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

    let request = Request::new(Method::GET, uri);
    let client = &data.client;

    // TODO: hot mess: needs cleaning
    match client.execute(request).await {
        Ok(res) => {
            let ln = res.content_length().unwrap() as usize;
            let content = &res.bytes().await.unwrap()[..];
            let mut w = BufWriter::new(vec![0u8; ln]);
            let mut r = BufReader::new(content);
            io::copy(&mut r, &mut w).unwrap();
            let body = w.into_inner().unwrap();
            HttpResponse::Ok().body(body)
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("Error requesting path: {}", e)),
    }
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
        .unwrap();

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

    future::try_join(s_in, s_out).await?;
    Ok(())
}
