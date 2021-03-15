use log::debug;
use rusqlite::{Connection, NO_PARAMS};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Store {
    path: Option<String>,
    connection: Connection,
}

static INITED: AtomicBool = AtomicBool::new(false);

impl Clone for Store {
    fn clone(&self) -> Self {
        Store::new(self.path.clone())
    }
}

impl Store {
    pub fn new(path: Option<String>) -> Store {
        let connection = if let Some(p) = &path {
            Connection::open(p).unwrap()
        } else {
            Connection::open_in_memory().unwrap()
        };
        let store = Store { connection, path };
        store.init();
        store
    }

    #[cfg(test)]
    pub fn in_memory() -> Store {
        Store::new(None)
    }

    pub fn init(&self) {
        if INITED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst) == Ok(false) {
            debug!("init store");
            self.connection
                .execute(
                    r#"CREATE TABLE IF NOT EXISTS chat (
                        id VARCHAR(100) PRIMARY KEY
                    )"#,
                    NO_PARAMS,
                )
                .unwrap();
        }
    }

    pub fn add_bot_to_chat(&self, chat_id: &str) {
        debug!("add bot to chat: {}", chat_id);
        let _ = self
            .connection
            .execute("INSERT OR IGNORE INTO chat (id) VALUES (?)", &[chat_id])
            .unwrap();
    }

    pub fn remove_bot_from_chat(&self, chat_id: &str) {
        debug!("remove bot from chat: {}", chat_id);
        let _ = self
            .connection
            .execute("DELETE FROM chat WHERE id = ?", &[chat_id])
            .unwrap();
    }

    pub fn exist_chat(&self, chat_id: &str) -> bool {
        debug!("exist chat: {}", chat_id);
        let count: isize = self
            .connection
            .query_row(
                "SELECT count(0) FROM chat WHERE id = ?",
                &[chat_id],
                |row| row.get(0),
            )
            .unwrap();
        count > 0
    }

    pub fn mail_for_chat(&self, chat_id: &str) -> Option<String> {
        debug!("mail for chat: {}", chat_id);
        if self.exist_chat(chat_id) {
            Some(format!("{}@mail.xcf.io", chat_id))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::store::Store;

    #[test]
    fn test_add_or_remove_bot() {
        let store = Store::in_memory();
        let chat_id = "some_chat_name";
        store.add_bot_to_chat(chat_id);
        assert!(store.exist_chat(chat_id));
        store.remove_bot_from_chat(chat_id);
        assert!(!store.exist_chat(chat_id))
    }
}
