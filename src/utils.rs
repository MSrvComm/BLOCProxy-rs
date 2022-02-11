use std::io::{BufWriter, BufReader, self};

use actix_web::{HttpResponse, error};
use reqwest::{Response, Client, Url};
use thiserror::Error;

#[derive(Clone)]
pub(crate) struct OutUrl {
    pub(crate) url: Url,
    pub(crate) out_client: Client,
}

#[derive(Clone)]
pub(crate) struct InClient {
    pub(crate) in_client: Client,
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

/// Helper function to extract body from response
/// and fill in a new request (or error)
pub(crate) async fn respond(res: Response) -> HttpResponse {
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
    match w.into_inner() {
        Ok(body) => HttpResponse::Ok().body(body),
        Err(e) => HttpResponse::from(error::ErrorInternalServerError(HotMess::FailedBufFlush(
            e.to_string(),
        ))),
    }
}