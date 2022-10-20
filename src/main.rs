use std::{convert::Infallible, str::FromStr, time::Duration};

use hyper::{
    body::Bytes,
    service::{make_service_fn, service_fn},
    Body, Client, Method, Request, Response,
};
use tracing::{error, info};
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    real_main().await.unwrap()
}

async fn real_main() -> color_eyre::Result<()> {
    color_eyre::install().unwrap();

    let filter_layer =
        Targets::from_str(std::env::var("RUST_LOG").as_deref().unwrap_or("info")).unwrap();
    let format_layer = tracing_subscriber::fmt::layer();
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(format_layer)
        .init();

    let h2_only: bool = std::env::var("H2_ONLY").map(|s| s == "1").unwrap_or(false);
    let h2_max_streams: u32 = std::env::var("H2_MAX_STREAMS")
        .map(|s| s.parse().unwrap())
        .unwrap_or(20);
    let h2_requests = std::env::var("H2_REQUESTS")
        .map(|s| s.parse().unwrap())
        .unwrap_or(30);

    info!("H2_ONLY={h2_only}, H2_MAX_STREAMS={h2_max_streams}, H2_REQUESTS={h2_requests}");
    info!("(Use environment variables to adjust)");

    let addr = "[::]:6400".parse()?;
    let server = hyper::server::Server::bind(&addr)
        .http2_max_concurrent_streams(h2_max_streams)
        .http2_only(h2_only)
        .serve(make_service_fn(|_conn| async {
            Ok::<_, Infallible>(service_fn(sample_endpoint))
        }));

    let _server_jh = tokio::spawn(async move {
        server.await.unwrap();
    });

    info!("Listening on {addr:?}");

    let client = Client::builder().http2_only(h2_only).build_http::<Body>();

    let (tx, mut rx) = tokio::sync::mpsc::channel(4096);

    let body = Bytes::from(vec![0u8; 65535 + 1]);

    for _ in 0..h2_requests {
        let req = Request::builder()
            .uri("http://localhost:6400")
            .method(Method::POST)
            .body(Body::from(body.clone()))?;
        let tx = tx.clone();
        let client = client.clone();
        tokio::spawn(async move {
            let res = client.request(req).await.unwrap();
            let _body = hyper::body::to_bytes(res.into_body()).await.unwrap();
            _ = tx.send(()).await;
        });
    }
    drop(tx);

    let mut complete_reqs = 0;

    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
        complete_reqs += 1;
    }

    if complete_reqs != h2_requests {
        error!("Stuck at {complete_reqs} / {h2_requests}");
    } else {
        info!("Completed {complete_reqs} / {h2_requests}");
    }

    Ok(())
}

async fn sample_endpoint(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let (_parts, req_body) = req.into_parts();
    hyper::body::to_bytes(req_body).await.unwrap();

    let res = Response::new("hi there".into());
    Ok(res)
}
