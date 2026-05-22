use std::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub enum ServerEvent {
    Request {
        method: String,
        path: String,
        status: u16,
    },
    Chat {
        user: String,
        text: String,
    },
    Listener {
        addr: String,
    },
}

pub type EventSender = Sender<ServerEvent>;

pub fn emit(tx: &Option<EventSender>, event: ServerEvent) {
    if let Some(tx) = tx {
        let _ = tx.send(event);
    }
}
