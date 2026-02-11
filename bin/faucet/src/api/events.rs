use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::WatchStream;

use crate::api::ApiServer;

/// SSE endpoint that streams issuance updates to connected clients.
///
/// Each client receives the current issuance value immediately on connection, then subsequent
/// updates whenever a mint transaction changes it. The event format is:
///
/// ```text
/// event: issuance
/// data: 12345
/// ```
pub async fn issuance_stream(
    State(server): State<ApiServer>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = WatchStream::new(server.issuance_receiver)
        .map(|value| Ok(Event::default().event("issuance").data(value.base_units().to_string())));

    Sse::new(stream).keep_alive(KeepAlive::new())
}
