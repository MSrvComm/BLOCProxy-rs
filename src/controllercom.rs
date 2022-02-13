pub(crate) async fn get_endpoints(svc: String) {
    // println!("{}", reqwest::get(format!("http://epwatcher:62000/{}", svc))
    println!("{}", reqwest::get(format!("http://localhost:30000/{}", svc))
        .await
        .unwrap()
        .text()
        .await
        .unwrap());
}
