use tokio::sync::{broadcast, mpsc, Mutex};

#[derive(Debug)]
pub struct InboundMessage {
    pub session_key: String,
    pub channel_name: String,
    pub user_id: String,
    pub user_name: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub session_key: String,
    pub text: String,
}

pub struct MessageBus {
    inbound_tx: mpsc::Sender<InboundMessage>,
    inbound_rx: Mutex<mpsc::Receiver<InboundMessage>>,
    outbound_tx: broadcast::Sender<OutboundMessage>,
}

impl MessageBus {
    pub fn new(buffer: usize) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel(buffer);
        let (outbound_tx, _) = broadcast::channel(buffer);
        Self {
            inbound_tx,
            inbound_rx: Mutex::new(inbound_rx),
            outbound_tx,
        }
    }

    pub fn inbound_sender(&self) -> mpsc::Sender<InboundMessage> {
        self.inbound_tx.clone()
    }

    pub async fn recv_inbound(&self) -> Option<InboundMessage> {
        self.inbound_rx.lock().await.recv().await
    }

    pub fn outbound_subscriber(&self) -> broadcast::Receiver<OutboundMessage> {
        self.outbound_tx.subscribe()
    }

    pub fn send_outbound(&self, msg: OutboundMessage) {
        let _ = self.outbound_tx.send(msg);
    }
}
