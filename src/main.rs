use std::{
    collections::HashMap,
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{ConnectInfo, Path, Query, Request, State},
    middleware::{self, Next},
    response::IntoResponse,
    routing,
};
use http::{StatusCode, header};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind};
use rmcp::transport::{
    SseServer,
    sse_server::SseServerConfig,
    streamable_http_server::axum::{StreamableHttpServer, StreamableHttpServerConfig},
};

use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, time::sleep};
use tokio_util::sync::CancellationToken;
use tower::{Service, ServiceBuilder};
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer, ExposeHeaders};
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod error;
mod gate;
mod orphan;
mod serde;
use config::{Config, McpServerConfig};
use gate::Gate;
use orphan::*;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Set the host to listen on. Default is `0.0.0.0`.
    #[arg(short = 'H', long, default_value = None)]
    host: Option<IpAddr>,

    /// Set the port to listen on. Default is 8051.
    #[arg(short = 'P', long, default_value_t = 8051)]
    port: u16,

    /// Set the configuration file to use. Default is ./config.json.
    #[arg(short = 'C', long, default_value = None)]
    conf: Option<PathBuf>,

    /// Enable Server-Sent Events. Default is false.
    #[arg(long)]
    sse: bool,
}
struct App {
    conf_path: PathBuf,
    bind_address: SocketAddr,
    sse: bool,
    config: Arc<RwLock<Arc<Config>>>,
    routers: Arc<RwLock<HashMap<Arc<str>, Router>>>,
    ct: CancellationToken,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let conf_path = args.conf.unwrap_or(PathBuf::from("./config.json"));

    let config: Arc<Config> = Arc::new(Config::read(&conf_path)?);

    let bind_address =
        SocketAddr::new(args.host.unwrap_or(Ipv4Addr::UNSPECIFIED.into()), args.port);

    let ct = CancellationToken::new();

    let app = Arc::new(App {
        conf_path: conf_path.clone(),
        sse: args.sse,
        bind_address,
        config: Arc::new(RwLock::new(config.clone())),
        routers: Default::default(),
        ct: ct.clone(),
    });

    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.blocking_send(res);
        },
        notify::Config::default()
            .with_poll_interval(Duration::from_secs(2))
            .with_compare_contents(true),
    )?;
    watcher.watch(conf_path.as_ref(), RecursiveMode::Recursive)?;

    {
        let app = app.clone();
        tokio::spawn(async move {
            let mut i = 0;
            let mut reload = None;
            loop {
                let rev = match reload.take() {
                    Some(mut wait) => {
                        tokio::select! {
                            _ = &mut wait => {
                                tracing::info!("config changed, reloading... {i}");
                                let _ = app.reload_config().await;
                                tracing::info!("config changed, reloaded {i}");
                                continue
                            },
                            res = rx.recv() => {
                                reload = Some(wait);
                                res
                            },
                        }
                    }
                    None => rx.recv().await,
                };
                i += 1;
                let Some(res) = rev else {
                    break;
                };

                let Ok(evt) = res else {
                    continue;
                };

                if matches!(evt.kind, EventKind::Modify(ModifyKind::Data(_))) {
                    reload = Some(Box::pin(sleep(Duration::from_secs(2))))
                }
            }
        });
    }

    let listener = tokio::net::TcpListener::bind(bind_address).await?;

    let router = Router::new()
        .route("/{service_name}", routing::any(serve_mcp))
        .route("/{service_name}/{*x}", routing::any(serve_mcp))
        .route("/mcp/config.json", routing::get(list_servers))
        .route("/mcp/config", routing::get(list_servers));

    let router = router.with_state(app);

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods(AllowMethods::any())
        .allow_headers(AllowHeaders::any())
        .expose_headers(ExposeHeaders::any());

    let service = router
        .layer(ServiceBuilder::new().layer(cors))
        .layer(middleware::from_fn(handle_logging))
        .into_make_service_with_connect_info::<SocketAddr>();

    let ct1 = ct.clone();
    let server = axum::serve(listener, service).with_graceful_shutdown(async move {
        ct1.cancelled().await;
        tracing::info!("streamable http server cancelled");
    });

    tokio::spawn(
        async move {
            tracing::info!("starting streamable http server on {}", bind_address);
            if let Err(e) = server.await {
                tracing::error!(error = %e, "streamable http server shutdown with error");
            }
        }
        .instrument(tracing::info_span!("streamable-http-server", bind_address = %bind_address)),
    );

    tokio::signal::ctrl_c().await?;
    ct.cancel();
    Ok(())
}

impl App {
    async fn reload_config(&self) -> anyhow::Result<()> {
        let new_config = Config::read(&self.conf_path)?;

        let mut removed = {
            self.routers
                .read()
                .await
                .keys()
                .filter(|k| !new_config.servers.contains_key(*k))
                .cloned()
                .collect::<Vec<_>>()
        };

        let removed2 = self.config.read().await.servers.iter().filter(|(n, server)| {
            matches!(new_config.servers.get(*n), Some(new_server) if *server != new_server)
        }).map(|(n, _)| n).cloned().collect::<Vec<_>>();

        removed.extend(removed2);

        let mut routers = self.routers.write().await;
        for n in removed {
            routers.remove(&n);
        }

        *self.config.write().await = Arc::new(new_config);
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
struct ListData<T> {
    count: usize,
    data: Vec<T>,
}

impl<T> ListData<T> {
    fn new(data: Vec<T>) -> Self {
        Self {
            count: data.len(),
            data,
        }
    }
}

impl<T> From<Vec<T>> for ListData<T> {
    fn from(data: Vec<T>) -> Self {
        Self::new(data)
    }
}

async fn handle_logging(
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let addr = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0)
        .map(|s| s.to_string());

    let res = next.run(req).await;
    if res.status().is_success() {
        tracing::info!(
            "{} {} {} {} {}",
            addr.unwrap_or_default(),
            method,
            path,
            res.status().as_str(),
            res.status(),
        );
    } else {
        tracing::info!(
            "{} {} {} {} {}",
            addr.unwrap_or_default(),
            method,
            path,
            res.status().as_str(),
            res.status(),
        );
    }
    Ok(res)
}

async fn list_servers(
    Query(params): Query<HashMap<String, String>>,
    State(app): State<Arc<App>>,
    req: Request,
) -> Json<Config> {
    let sse = params.contains_key("sse");
    let schame = if params.contains_key("https") {
        "https"
    } else {
        "http"
    };
    let host = params.get("host").map(|s| s.as_str());

    let host = host.unwrap_or_else(|| {
        req.headers()
            .get(header::HOST)
            .iter()
            .flat_map(|x| x.to_str())
            .next()
            .unwrap_or_default()
    });

    let servers = app
        .config
        .read()
        .await
        .servers
        .iter()
        .map(|(name, s)| {
            let config = if sse {
                s.to_sse(format!("{schame}://{host}/{name}/sse"))
            } else {
                s.to_streamable(format!("{schame}://{host}/{name}"))
            };

            (name.clone(), Arc::new(config))
        })
        .collect();

    let config = Config { servers };

    Json(config)
}

async fn serve_mcp(
    Path(params): Path<HashMap<String, String>>,
    State(app): State<Arc<App>>,
    req: Request,
) -> Result<axum::http::Response<axum::body::Body>, Infallible> {
    use axum::response::IntoResponse;
    let service_name: Arc<str> = params.get("service_name").unwrap().to_string().into();

    let path_prefix = {
        let path = req.uri().path().to_string();
        path[0..(path.find(service_name.as_ref()).unwrap_or_default() + service_name.len())]
            .to_string()
    };

    let router = app.routers.read().await.get(&service_name).cloned();

    let router = match router {
        Some(router) => router,
        None => {
            let Some(config) = app.config.read().await.servers.get(&service_name).cloned() else {
                return Ok((
                    StatusCode::NOT_FOUND,
                    format!("Service {service_name} not found"),
                )
                    .into_response());
            };

            let router = make_mcp_router(
                &service_name,
                config,
                app.sse,
                app.bind_address,
                app.ct.clone(),
            );
            app.routers
                .write()
                .await
                .insert(service_name, router.clone());
            router
        }
    };
    let res = Router::new().nest(&path_prefix, router).call(req).await?;
    Ok(res)
}

fn make_mcp_router(
    name: &str,
    server_config: Arc<McpServerConfig>,
    sse: bool,
    bind_address: SocketAddr,
    ct: CancellationToken,
) -> Router {
    let mut service_router = Router::new();
    if sse {
        let (sse_server, sse_router) = SseServer::new_with_custom_post_path(
            SseServerConfig {
                bind: bind_address,
                sse_path: "/sse".to_string(),
                post_path: "/message".to_string(),
                ct: ct.clone(),
                sse_keep_alive: None,
            },
            format!("/{name}/message"),
        );

        sse_server.with_service({
            let server_config = server_config.clone();
            move || Gate::new(server_config.clone())
        });

        service_router = service_router.merge(sse_router)
    }

    let streamable_router = {
        let (streamable_http_server, streamable_router) =
            StreamableHttpServer::new(StreamableHttpServerConfig {
                bind: bind_address,
                ct: ct.clone(),
                ..Default::default()
            });

        streamable_http_server.with_service({
            let server_config = server_config.clone();
            move || Gate::new(server_config.clone())
        });

        streamable_router
    };

    service_router = service_router.merge(streamable_router);

    service_router
}
