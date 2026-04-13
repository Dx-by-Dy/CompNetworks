use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header::HOST;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode, Uri};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto;
use std::convert::Infallible;
use tokio::net::TcpListener;
use tracing::{error, info};

type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

#[tokio::main]
async fn main() {
    let file_appender = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("proxy.log")
        .unwrap();

    tracing_subscriber::fmt()
        .with_writer(file_appender)
        .with_ansi(false)
        .init();

    let listener = TcpListener::bind("127.0.0.1:8888").await.unwrap();
    let client: Client<HttpConnector, Incoming> =
        Client::builder(TokioExecutor::new()).build(HttpConnector::new());

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let client = client.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let service = service_fn(move |req| proxy_handler(req, client.clone()));

            if let Err(err) = auto::Builder::new(TokioExecutor::new())
                .serve_connection(io, service)
                .await
            {
                eprintln!("Connection error: {err}");
            }
        });
    }
}

async fn proxy_handler(
    mut req: Request<Incoming>,
    client: Client<HttpConnector, Incoming>,
) -> Result<Response<BoxBody>, Infallible> {
    match *req.method() {
        Method::GET | Method::POST => {}
        _ => {
            return Ok(simple_response(
                StatusCode::METHOD_NOT_ALLOWED,
                "Only GET and POST supported",
            ));
        }
    }

    let Some(path) = req
        .uri()
        .path_and_query()
        .and_then(|pq| pq.as_str().strip_prefix("/"))
        .map(|h| h.to_string())
    else {
        return Ok(simple_response(
            StatusCode::BAD_REQUEST,
            "Expected query like 'http://example.com/'",
        ));
    };
    let Some(host) = path.split('/').nth(2).and_then(|h| h.split("?").next()) else {
        return Ok(simple_response(
            StatusCode::BAD_REQUEST,
            &format!("Expected query like 'http://example.com/', your query: '{path}'"),
        ));
    };
    let Ok(new_uri) = path.parse::<Uri>() else {
        return Ok(simple_response(
            StatusCode::BAD_REQUEST,
            &format!("Expected query like 'http://example.com/', your query: '{path}'"),
        ));
    };
    let Some(header_host) = host.parse().ok() else {
        return Ok(simple_response(
            StatusCode::BAD_REQUEST,
            &format!("Expected query like 'http://example.com/', your query: '{path}'"),
        ));
    };

    req.headers_mut().insert(HOST, header_host);
    *req.uri_mut() = new_uri;

    let result = client.request(req).await;
    match result {
        Ok(res) => {
            info!(
                query = %path,
                status = %res.status(),
            );

            let boxed = res.map(|b| b.boxed());
            Ok(boxed)
        }
        Err(err) => {
            error!(
                query = %path,
                err = %err,
            );

            Ok(simple_response(
                StatusCode::BAD_GATEWAY,
                &format!("Upstream request failed with error: {}", err),
            ))
        }
    }
}

fn simple_response(status: StatusCode, msg: &str) -> Response<BoxBody> {
    let body = Full::new(Bytes::from(msg.to_string()))
        .map_err(|never| match never {})
        .boxed();

    Response::builder().status(status).body(body).unwrap()
}
