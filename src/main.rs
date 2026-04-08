use std::{io::Error as IoError, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpSocket, TcpStream},
    signal,
    sync::{
        Semaphore,
        broadcast::{self, Receiver},
        mpsc::{self, Sender},
    },
    time::timeout,
};
use tracing::{debug, error, info, instrument};
use tracing_subscriber::{
    self, EnvFilter,
    fmt::{self, format::FmtSpan},
};

use mini_redis::{
    Connection, ConnectionError, Frame, Result, api,
    cmd::Command,
    config::Config,
    db::{Db, purge_expired_tasks, run_trash_janitor},
    metrics::{self, ActiveConnectionGuard},
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load().expect("Failed to load configurations from environment variable");

    let format = fmt::format().with_level(true).with_target(true).compact();
    let filter = EnvFilter::from_default_env();

    tracing_subscriber::fmt()
        .event_format(format)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_env_filter(filter)
        .init();

    info!("rust app starting up...");

    let (db, rx) = Db::new();
    let db = Arc::new(db);
    let janitor_handle = tokio::spawn(run_trash_janitor(rx));

    let guard = Arc::new(Semaphore::new(config.max_connections));

    let addr = config.redis_bind_addr.parse().unwrap();

    let socket = TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?;
    socket.bind(addr)?;

    let listener = socket.listen(config.listen_backlog)?;

    info!("Server listening on port {}", config.redis_bind_addr);

    let (notify_shutdown_tx, _) = broadcast::channel::<()>(config.max_connections);
    let (shutdown_complete_tx, mut shutdown_complete_rx) =
        mpsc::channel::<()>(config.max_connections);

    let notify_shutdown_tx_clone = notify_shutdown_tx.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("Shutdown signal received, stopping server...");
        let _ = notify_shutdown_tx_clone.send(());
    });

    tokio::spawn(purge_expired_tasks(
        db.clone(),
        notify_shutdown_tx.subscribe(),
    ));

    let redis_tx = shutdown_complete_tx.clone();
    let api_tx = shutdown_complete_tx.clone();
    let redis_rx = notify_shutdown_tx.subscribe();
    let api_rx = notify_shutdown_tx.subscribe();

    let redis_config = config.clone();
    let api_config = config.clone();

    tokio::select! {
        _ = run_redis_server(listener, guard, db, redis_tx, redis_rx, redis_config) => {},
        _ = api::run_api_server(api_tx, api_rx, api_config) => {},
    }

    drop(notify_shutdown_tx);
    drop(shutdown_complete_tx);

    let _ = shutdown_complete_rx.recv().await;
    let _ = janitor_handle.await;
    info!("Server shutting down gracefully");
    Ok(())
}

async fn run_redis_server(
    listener: TcpListener,
    guard: Arc<Semaphore>,
    db: Arc<Db>,
    shutdown_complete_tx: Sender<()>,
    mut notify_shutdown_rx: Receiver<()>,
    config: Config,
) -> Result<()> {
    let mut timeout = Duration::from_millis(1);

    loop {
        let result = tokio::select! {
            _ = notify_shutdown_rx.recv() => { return Ok(()); },
            result = listener.accept() => { result }
        };

        match result {
            Ok((stream, _)) => {
                timeout = Duration::from_millis(1);

                if let Ok(addr) = stream.peer_addr() {
                    debug!(addr = %addr, "Accepted new connection.");
                }

                let _tx_clone = shutdown_complete_tx.clone();
                let _rx_clone = notify_shutdown_rx.resubscribe();

                let db_ref = db.clone();
                let guard_clone = guard.clone();
                let config_clone = config.clone();
                tokio::spawn(async move {
                    if let Ok(_permit) = guard_clone.acquire_owned().await {
                        let _guard = ActiveConnectionGuard::new();
                        let _ = handle_client(
                            stream,
                            db_ref,
                            _rx_clone,
                            _tx_clone,
                            config_clone.idle_timeout(),
                        )
                        .await;
                    };
                });
            }
            Err(e) => {
                error!("Unable to connect: {:?}", e);
                if is_recoverable_connection_error(&e) {
                    continue;
                }
                error!("Accept failed, entering backoff state");
                tokio::time::sleep(timeout).await;
                if timeout < Duration::from_secs(64) {
                    timeout *= 2;
                }
            }
        }
    }
}

fn is_recoverable_connection_error(e: &IoError) -> bool {
    let errorkind = e.kind();

    matches!(
        errorkind,
        std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
    )
}

// Handle client functions
#[instrument(level = "trace", skip_all)]
async fn handle_client(
    stream: TcpStream,
    db: Arc<Db>,
    mut notify_shutdown_rx: Receiver<()>,
    _shutdown_complete_tx: Sender<()>,
    idle_timeout: Duration,
) -> Result<()> {
    let mut connection = Connection::new(stream);
    loop {
        let (timeout_result, parse_timer) = tokio::select! {
            _ = notify_shutdown_rx.recv() => { return Ok(()); },
            timeout_result = timeout(idle_timeout, connection.read_frame()) => {
                let _parse_timer = metrics::PARSE_LATENCY.start_timer();
                (timeout_result, _parse_timer)
            }
        };

        let request_frame = timeout_result
            .map_err(|_| ConnectionError::IdleTimeout)?
            .map_err(|_| ConnectionError::ResetByPeer)?;

        let Some(frame) = request_frame else {
            return Ok(());
        };

        drop(parse_timer);

        metrics::COUNTER.inc();

        let response_result = handle_command(frame, &db).await;

        let response_frame = match response_result {
            Ok(frame) => frame,
            Err(e) => Frame::Error(e.to_string()),
        };

        {
            let _write_timer = metrics::WRITE_LATENCY.start_timer();
            connection.write_frame(&response_frame).await?;
        }
    }
}

#[instrument(level = "trace", skip(db))]
async fn handle_command(value: Frame, db: &Db) -> Result<Frame> {
    let command = Command::from_frame(value)?;
    {
        let _process_timer = metrics::PROCESS_LATENCY
            .with_label_values(&[command.name()])
            .start_timer();
        command.apply(db).await
    }
}
