use actix_web::{http::HeaderMap, web, HttpRequest, HttpResponse, Responder};
use reqwest::{Method, Request, Url};

use crate::{
    mgr::Service,
    utils::{respond, OutData},
};

pub(crate) async fn get_out(req: HttpRequest, data: web::Data<OutData>) -> impl Responder {
    let mut headermap = HeaderMap::new();
    for (key, val) in req.headers() {
        headermap.insert(key.clone(), val.clone());
    }

    println!("remote hostname: {:#?}", req.connection_info().host());
    let path_str = &req.path().to_owned();
    let path = String::from_utf8_lossy(&path_str.as_bytes()[1..]);

    let conn = req.connection_info();
    let svc = match conn.host().split(":").nth(0) {
        Some(svc) => svc,
        None => {
            return HttpResponse::InternalServerError().body(format!("Invalid service requested"))
        }
    };

    println!("service: {}", svc);

    let query = req.query_string();
    let uri = match query {
        "" => Url::parse(&format!("{}{}", data.url.as_str(), path)).unwrap(),
        _ => Url::parse(&format!(
            "{}{}?{}",
            data.url.as_str(),
            path,
            req.query_string()
        ))
        .unwrap(),
    };
    println!("outgoing uri: {}", uri);

    let client = &data.out_client.clone();
    let request = Request::new(Method::GET, uri);

    // get backends
    let backends = Service::get_backends(svc);
    println!("{:#?}", backends);

    let handle = async_std::task::spawn(client.execute(request));
    let res = match handle.await {
        Ok(res) => res,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error requesting path: {}", e))
        }
    };

    respond(res).await
}
