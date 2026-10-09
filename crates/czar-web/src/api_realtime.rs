use axum::{
    extract::{
        State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::sink::Sink;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use tokio::sync::broadcast;

use crate::api::{ApiServer, StateUpdate};

pub(crate) async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(api): State<ApiServer>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, api))
}

async fn websocket_connection(socket: WebSocket, api: ApiServer) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = api.broadcast_tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        let _ = pump_broadcast_updates(&mut sender, &mut rx).await;
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(_msg)) = receiver.next().await {
            // Client messages ignored for now (could add ping/pong here)
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}

async fn pump_broadcast_updates<S>(
    sender: &mut S,
    rx: &mut broadcast::Receiver<StateUpdate>,
) -> Result<(), S::Error>
where
    S: Sink<axum::extract::ws::Message> + Unpin,
{
    use axum::extract::ws::Message;

    while let Ok(update) = rx.recv().await {
        let json = serde_json::to_string(&update).unwrap_or_default();
        if sender.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
