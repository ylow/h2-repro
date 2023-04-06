use color_eyre::eyre;
use futures::prelude::stream::*;
use hyper::{
    body::Bytes,
    client::HttpConnector,
    service::{make_service_fn, service_fn},
    Body, Client, Method, Request, Response,
};
use std::{convert::Infallible, net::TcpListener, str::FromStr, time::Duration};
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    real_main().await.unwrap()
}

async fn real_main() -> eyre::Result<()> {
    color_eyre::install().unwrap();

    let filter_layer =
        Targets::from_str(std::env::var("RUST_LOG").as_deref().unwrap_or("info")).unwrap();
    let format_layer = tracing_subscriber::fmt::layer();
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(format_layer)
        .init();

    let h2_max_streams: u32 = std::env::var("H2_MAX_STREAMS")
        .map(|s| s.parse().unwrap())
        .unwrap_or(100);
    let h2_requests = std::env::var("H2_REQUESTS")
        .map(|s| s.parse().unwrap())
        .unwrap_or(6000);

    info!("{h2_requests} requests on {h2_max_streams} streams");
    info!("(Set $H2_REQUESTS and $H2_MAX_STREAMS environment variables to adjust)");

    run_test(true, h2_max_streams, h2_requests).await?;

    Ok(())
}

async fn run_test(h2_only: bool, h2_max_streams: u32, h2_requests: u32) -> eyre::Result<()> {
    let prefix = if h2_only { "H2" } else { "H1" };

    let ln = TcpListener::bind("[::]:0")?;
    let addr = ln.local_addr()?;
    let server = hyper::server::Server::from_tcp(ln)?
        .http2_max_concurrent_streams(h2_max_streams)
        .http2_only(h2_only)
        .serve(make_service_fn(|_conn| async {
            Ok::<_, Infallible>(service_fn(sample_endpoint))
        }));

    let _server_jh = tokio::spawn(async move {
        server.await.unwrap();
    });

    let client = Client::builder().http2_only(h2_only).build_http::<Body>();

    let (tx, mut rx) = mpsc::channel::<eyre::Result<()>>(4096);

    let body = Bytes::from(vec![0u8; 65535 + 1]);

    async fn do_one_request(req: Request<Body>, client: Client<HttpConnector>) -> eyre::Result<()> {
        let res = client.request(req).await?;
        _ = hyper::body::to_bytes(res.into_body()).await?;
        Ok(())
    }
    let mut strm = iter((0..h2_requests).map(|_| {
        let req = Request::builder()
            .uri(format!("http://{addr}"))
            .method(Method::POST)
            .body(Body::from(body.clone()))
            .unwrap();
        do_one_request(req, client.clone())
    }))
    .buffered(100);

    let mut ctr: u64 = 0;
    while let Some(resp) = strm.next().await {
        ctr += 1;
        eprintln!("{ctr} done");
        let _ = tx.send(resp).await;
    }
    // everything after this is irrelevant if hangs

    /*
    for _ in 0..h2_requests {
        let req = Request::builder()
            .uri(format!("http://{addr}"))
            .method(Method::POST)
            .body(Body::from(body.clone()))?;
        let fut = do_one_request(req, client.clone());
        let tx = tx.clone();
        tokio::spawn(async move { _ = tx.send(fut.await).await });
    }*/
    drop(tx);

    let mut complete_reqs = 0;

    while let Ok(Some(res)) = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
        res?;
        complete_reqs += 1;
    }

    if complete_reqs != h2_requests {
        error!("{prefix}: Stuck at {complete_reqs} / {h2_requests}");
    } else {
        info!("{prefix}: Completed {complete_reqs} / {h2_requests}");
    }

    Ok(())
}

async fn sample_endpoint(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let (_parts, req_body) = req.into_parts();
    hyper::body::to_bytes(req_body).await.unwrap();

    let res = Response::new("hi there".into());
    Ok(res)
}
