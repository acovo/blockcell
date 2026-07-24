use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use tokio::sync::oneshot;

pub const MESSAGE_RECEIPT_ID: &str = "message_receipt_id";

type ReceiptResult = std::result::Result<(), String>;
type ReceiptSender = oneshot::Sender<ReceiptResult>;

fn receipt_registry() -> &'static Mutex<HashMap<String, ReceiptSender>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, ReceiptSender>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_message_receipt() -> (String, oneshot::Receiver<ReceiptResult>) {
    let receipt_id = uuid::Uuid::new_v4().simple().to_string();
    let (sender, receiver) = oneshot::channel();
    receipt_registry()
        .lock()
        .expect("message receipt registry poisoned")
        .insert(receipt_id.clone(), sender);
    (receipt_id, receiver)
}

pub fn complete_message_receipt(receipt_id: &str, result: ReceiptResult) -> bool {
    let sender = receipt_registry()
        .lock()
        .expect("message receipt registry poisoned")
        .remove(receipt_id);
    sender.is_some_and(|sender| sender.send(result).is_ok())
}

pub fn cancel_message_receipt(receipt_id: &str) -> bool {
    receipt_registry()
        .lock()
        .expect("message receipt registry poisoned")
        .remove(receipt_id)
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn registered_receipt_receives_completion_once() {
        let (receipt_id, receiver) = register_message_receipt();

        assert!(complete_message_receipt(&receipt_id, Ok(())));
        assert!(!complete_message_receipt(&receipt_id, Ok(())));
        assert_eq!(receiver.await.expect("receipt sender"), Ok(()));
    }

    #[tokio::test]
    async fn cancelled_receipt_drops_receiver() {
        let (receipt_id, receiver) = register_message_receipt();

        assert!(cancel_message_receipt(&receipt_id));
        assert!(receiver.await.is_err());
    }
}
