use std::{convert::Infallible, str::FromStr, time::Duration};

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Client, Method, Request, Response,
};
use tracing::info;
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

    let addr = "[::]:6400".parse()?;
    let server = hyper::server::Server::bind(&addr)
        .http2_only(true)
        .http2_max_concurrent_streams(100)
        .serve(make_service_fn(|_conn| async {
            Ok::<_, Infallible>(service_fn(sample_endpoint))
        }));

    let _server_jh = tokio::spawn(async move {
        server.await.unwrap();
    });

    info!("Listening on {addr:?}");

    let client = Client::builder().http2_only(true).build_http::<Body>();

    let (tx, mut rx) = tokio::sync::mpsc::channel(4096);

    let total_reqs = 200;
    for i in 0..total_reqs {
        let req = Request::builder()
            .uri("http://localhost:6400")
            .method(Method::POST)
            .body(vec![0u8; 256 * 1024].into())?;
        let tx = tx.clone();
        let client = client.clone();
        tokio::spawn(async move {
            let res = client.request(req).await.unwrap();
            let _body = hyper::body::to_bytes(res.into_body()).await.unwrap();
            _ = tx.send((i, ())).await;
        });
    }
    drop(tx);

    let mut complete_reqs = 0;
    while let Some((i, _res)) = rx.recv().await {
        complete_reqs += 1;
        info!("Received response {i} ({complete_reqs}/{total_reqs} done)");
    }

    Ok(())
}

async fn sample_endpoint(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let (_parts, req_body) = req.into_parts();
    hyper::body::to_bytes(req_body).await.unwrap();

    let (mut sender, res_body) = Body::channel();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        sender.send_data("hi there".into()).await.unwrap();
    });

    let res = Response::new(res_body);
    Ok(res)
}
