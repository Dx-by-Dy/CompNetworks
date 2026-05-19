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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{error, info};

type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

#[derive(Clone, Serialize, Deserialize)]
struct CacheEntry {
    path: String,
    etag: Option<String>,
    last_modified: Option<String>,
}

#[derive(Default)]
struct CacheIndex {
    map: HashMap<String, CacheEntry>,
}

#[derive(Deserialize)]
struct BlacklistConfig {
    blocked_domains: Vec<String>,
    blocked_urls: Vec<String>,
}

type SharedCache = Arc<Mutex<CacheIndex>>;

fn make_cache_key(method: &Method, uri: &Uri) -> String {
    use sha1::{Digest, Sha1};

    let key = format!("{}:{}", method, uri);
    format!("{:x}", Sha1::digest(key.as_bytes()))
}

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

    let cache: SharedCache = Arc::new(Mutex::new(CacheIndex::default()));
    tokio::fs::create_dir_all("cache").await.unwrap();

    let blacklist = Arc::new(load_blacklist().await);

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let client = client.clone();
        let cache_clone = cache.clone();
        let blacklist_clone = blacklist.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);

            let service = service_fn(move |req| {
                proxy_handler(
                    req,
                    client.clone(),
                    cache_clone.clone(),
                    blacklist_clone.clone(),
                )
            });

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
    cache: SharedCache,
    blacklist: Arc<BlacklistConfig>,
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

    // -------------- Достаем запрос из path and query -------------------------------------
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
    *req.uri_mut() = new_uri.clone();
    // -------------------------------------------------------------------------------------

    // -------------- Проверяем если запрещен доступ к uri ---------------------------------
    if is_blocked(&new_uri, &blacklist) {
        info!(
            uri = %new_uri,
            "blocked by blacklist",
        );

        return Ok(simple_response(
            StatusCode::FORBIDDEN,
            "Blocked by proxy blacklist",
        ));
    }
    // -------------------------------------------------------------------------------------

    // -------------- Проверяем если ли ключ, модифицируем и отправляем req ----------------
    let cache_key = make_cache_key(req.method(), &new_uri);
    if req.method() == Method::GET {
        let entry = {
            let cache_guard = cache.lock().await;
            cache_guard.map.get(&cache_key).cloned()
        };

        if let Some(entry) = entry {
            if let Some(etag) = entry.etag {
                req.headers_mut()
                    .insert("If-None-Match", etag.parse().unwrap());
            }

            if let Some(last_modified) = entry.last_modified {
                req.headers_mut()
                    .insert("If-Modified-Since", last_modified.parse().unwrap());
            }
        }
    }
    let result = client.request(req).await;
    // -------------------------------------------------------------------------------------

    match result {
        Ok(res) => {
            let status = res.status();

            info!(
                uri = %new_uri,
                status = %status,
            );

            // -------------- Отдаем файл из кеша если ответ не изменился ------------------
            if status == StatusCode::NOT_MODIFIED {
                let entry = {
                    let cache_guard = cache.lock().await;
                    cache_guard.map.get(&cache_key).cloned()
                };

                if let Some(entry) = entry {
                    match tokio::fs::read(&entry.path).await {
                        Ok(bytes) => {
                            let body = Full::new(Bytes::from(bytes))
                                .map_err(|never| match never {})
                                .boxed();

                            return Ok(Response::builder()
                                .status(StatusCode::OK)
                                .body(body)
                                .unwrap());
                        }
                        Err(err) => {
                            error!("cache read error: {}", err);
                        }
                    }
                }
            }
            // ------------------------------------------------------------------------------

            // -------------- Сохраняем в кэш, если получили ответ --------------------------
            let (parts, body) = res.into_parts();
            let collected = match body.collect().await {
                Ok(c) => c,
                Err(err) => {
                    return Ok(simple_response(
                        StatusCode::BAD_GATEWAY,
                        &format!("Body read error: {}", err),
                    ));
                }
            };

            let body_bytes = collected.to_bytes();
            if parts.status == StatusCode::OK && parts.status != StatusCode::NOT_MODIFIED {
                if new_uri.scheme_str() == Some("http") && parts.status == StatusCode::OK {
                    let file_path = format!("cache/{}.body", cache_key);

                    if let Err(err) = tokio::fs::write(&file_path, &body_bytes).await {
                        error!("cache write error: {}", err);
                    } else {
                        let etag = parts
                            .headers
                            .get("etag")
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.to_string());

                        let last_modified = parts
                            .headers
                            .get("last-modified")
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.to_string());

                        let entry = CacheEntry {
                            path: file_path,
                            etag,
                            last_modified,
                        };

                        let mut cache_guard = cache.lock().await;
                        cache_guard.map.insert(cache_key.clone(), entry);
                    }
                }
            }
            // ------------------------------------------------------------------------------

            let body = Full::new(body_bytes)
                .map_err(|never| match never {})
                .boxed();

            Ok(Response::from_parts(parts, body))
        }
        Err(err) => {
            error!(
                uri = %new_uri,
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

async fn load_blacklist() -> BlacklistConfig {
    let text = tokio::fs::read_to_string("blacklist.toml")
        .await
        .unwrap_or_default();

    toml::from_str(&text).unwrap_or(BlacklistConfig {
        blocked_domains: vec![],
        blocked_urls: vec![],
    })
}

fn is_blocked(uri: &Uri, blacklist: &BlacklistConfig) -> bool {
    let uri_str = uri.to_string();

    if blacklist
        .blocked_urls
        .iter()
        .any(|u| uri_str.starts_with(u))
    {
        return true;
    }

    if let Some(host) = uri.host() {
        if blacklist
            .blocked_domains
            .iter()
            .any(|d| host == d || host.ends_with(&format!(".{}", d)))
        {
            return true;
        }
    }
    
    false
}
