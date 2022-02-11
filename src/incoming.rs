use actix_web::{HttpRequest, web, HttpResponse, http::HeaderValue};
use reqwest::Url;

use crate::{utils::{InClient, respond}, PROXYINPORT, CLIENTPORT};

pub(crate) async fn get_in(req: HttpRequest, data: web::Data<InClient>) -> HttpResponse {
    println!("incoming on {}\n", PROXYINPORT);
    let host;
    match req.connection_info().remote_addr() {
        Some(s) => {
            match s.split(":").nth(0) {
                Some(h) => {
                    host = match HeaderValue::from_str(h) {
                        Ok(val) => val,
                        Err(e) => {
                            return HttpResponse::BadRequest().body(format!("{:#?}", e.to_string()))
                        }
                    }
                }
                None => return HttpResponse::BadRequest().body("Invalid Address"),
            };
        }
        None => return HttpResponse::BadRequest().body("Invalid Address"),
    };
    let uri = match Url::parse(&format!(
        "http://localhost:{}/{}",
        CLIENTPORT,
        &req.uri().to_string()[1..]
    )) {
        Ok(uri) => uri,
        Err(e) => return HttpResponse::BadRequest().body(format!("{:#?}", e.to_string())),
    };

    let handle = async_std::task::spawn(
        data.in_client
            .get(uri)
            .header("X-Forwarded-For", host)
            .send(),
    );

    let res = match handle.await {
        Ok(res) => res,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error requesting path: {}", e))
        }
    };

    respond(res).await
}