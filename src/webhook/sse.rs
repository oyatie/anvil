use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::webhook::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetEventMessage {
    pub event_type: String,
    pub repo: String,
    pub entity_id: String,
    pub title: String,
    pub status: String,
    pub timestamp_utc: String,
    pub payload_json: Option<String>,
}

#[derive(Clone)]
pub struct FleetEventBroadcaster {
    sender: Sender<FleetEventMessage>,
}

impl Default for FleetEventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl FleetEventBroadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }

    pub fn broadcast_event(&self, event: FleetEventMessage) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> Receiver<FleetEventMessage> {
        self.sender.subscribe()
    }
}

/// Real-time Server-Sent Events (SSE) stream handler for connected Leptos clients
pub async fn sse_fleet_stream_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.broadcaster.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event) => {
            let json = serde_json::to_string(&event).unwrap_or_default();
            Some(Ok(Event::default().event("fleet_event").data(json)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
