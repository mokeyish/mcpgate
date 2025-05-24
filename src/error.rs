use std::io;

use rmcp::{
    service,
    transport::{self, sse_client, streamable_http_client},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("Client error: {0}")]
    SseTransport(#[from] transport::sse_client::SseTransportError<reqwest::Error>),
    #[error("{0}")]
    SseClientInitialize(
        #[from] service::ClientInitializeError<sse_client::SseTransportError<reqwest::Error>>,
    ),
    #[error("{0}")]
    StdioClientInitialize(#[from] service::ClientInitializeError<io::Error>),
    #[error("{0}")]
    StreamableClientInitialize(
        #[from]
        service::ClientInitializeError<
            streamable_http_client::StreamableHttpError<reqwest::Error>,
        >,
    ),
}
