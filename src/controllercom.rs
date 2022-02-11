pub(crate) async fn getEndpoints(svc: &str) {
    println!("{}", reqwest::get(format!("http://epwatcher:62000/{}", svc))
        .await
        .unwrap()
        .text()
        .await
        .unwrap());
}
