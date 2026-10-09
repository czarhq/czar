use super::*;
use axum::extract::ws::Message;
use futures::task::{Context, Poll};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Clone, Default)]
struct RecordingSink {
    fail_on_send: bool,
    sent: Arc<Mutex<Vec<Message>>>,
}

impl Sink<Message> for RecordingSink {
    type Error = std::io::Error;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        if self.fail_on_send {
            return Err(std::io::Error::other("send failed"));
        }
        self.sent.lock().expect("sent lock").push(item);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn test_pump_broadcast_updates_sends_and_stops_when_channel_closes() {
    let (tx, mut rx) = broadcast::channel(4);
    let mut sink = RecordingSink::default();
    tx.send(StateUpdate::RunUpdated {
        run_id: "run-1".to_string(),
        status: "running".to_string(),
    })
    .expect("send update");
    drop(tx);

    pump_broadcast_updates(&mut sink, &mut rx)
        .await
        .expect("pump updates");
    let sent = sink.sent.lock().expect("sent lock");
    assert_eq!(sent.len(), 1);
}

#[tokio::test]
async fn test_pump_broadcast_updates_breaks_on_send_error() {
    let (tx, mut rx) = broadcast::channel(4);
    let mut sink = RecordingSink {
        fail_on_send: true,
        sent: Arc::new(Mutex::new(Vec::new())),
    };
    tx.send(StateUpdate::RunUpdated {
        run_id: "run-2".to_string(),
        status: "running".to_string(),
    })
    .expect("send update");
    drop(tx);

    pump_broadcast_updates(&mut sink, &mut rx)
        .await
        .expect("pump should stop cleanly");
    let sent = sink.sent.lock().expect("sent lock");
    assert!(sent.is_empty());
}

#[tokio::test]
async fn test_recording_sink_close_calls_poll_close() {
    let mut sink = RecordingSink::default();
    sink.close().await.expect("close should succeed");
}
