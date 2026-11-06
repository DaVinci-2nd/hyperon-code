use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

pub struct Pending {
    map: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl Pending {
    pub fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    pub fn register(&self, id: &str) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        if let Ok(mut map) = self.map.lock() {
            map.insert(id.to_string(), tx);
        }
        rx
    }

    pub fn resolve(&self, id: &str, allowed: bool) -> bool {
        let sender = {
            let mut map = match self.map.lock() {
                Ok(g) => g,
                Err(_) => return false,
            };
            map.remove(id)
        };
        match sender {
            Some(tx) => tx.send(allowed).is_ok(),
            None => false,
        }
    }
}
