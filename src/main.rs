use std::{
    convert::Infallible,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response,
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
        .serve(make_service_fn(|_conn| async {
            Ok::<_, Infallible>(service_fn(hello_world))
        }));

    info!("Listening on {addr:?}");

    server.await?;

    Ok(())
}

struct State {
    counter: usize,
}

lazy_static::lazy_static! {
    static ref STATE: Arc<Mutex<State>> = Arc::new(Mutex::new(State { counter: 0 }));
}

async fn hello_world(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let (mut sender, body) = hyper::Body::channel();
    let counter = {
        let mut state = STATE.lock().unwrap();
        let counter = &mut state.counter;
        *counter += 1;
        *counter
    };

    if counter > 2 {
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            sender.send_data("this should work!".into()).await.unwrap();
        });
    } else {
        tokio::spawn(async move {
            for _ in 0..3 {
                sender.send_data("this will fail...".into()).await.unwrap();
                tokio::time::sleep(Duration::from_millis(00)).await;
            }
            sender.abort();
        });
    }

    let res = Response::new(body);
    Ok(res)
}
