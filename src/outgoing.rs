use actix_web::{http::HeaderMap, web, HttpRequest, HttpResponse, Responder};
use reqwest::{Method, Request, Url};

use crate::{
    controllercom::get_endpoints,
    utils::{respond, OutUrl},
};

pub(crate) async fn get_out(req: HttpRequest, data: web::Data<OutUrl>) -> impl Responder {
    let mut headermap = HeaderMap::new();
    for (key, val) in req.headers() {
        headermap.insert(key.clone(), val.clone());
    }

    let uri = &req.uri().to_string()[1..];
    let svc = match uri.split("/").nth(0) {
        Some(svc) => svc,
        None => return HttpResponse::BadRequest().body("Error extracting service"),
    };
    println!("svc: {}", svc);

    let uri = Url::parse(&format!("{}{}", data.url.as_str(), &uri)).unwrap();
    println!("outgoing uri: {}", uri);

    let client = &data.out_client.clone();
    let request = Request::new(Method::GET, uri);

    // how do you handle the error
    // when an async call like get_endpoints(svc.to_string()).await fails?
    // it won't crash but returns an empty error back
    async_std::task::spawn(get_endpoints(svc.to_string())).await;

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
