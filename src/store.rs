use anyhow::Result;
use log::{debug, error};
use rusqlite::{Connection, NO_PARAMS};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Store {
    path: Option<String>,
    connection: Connection,
    mail_domain: String,
}

static INITED: AtomicBool = AtomicBool::new(false);

impl Clone for Store {
    fn clone(&self) -> Self {
        Store::new(self.path.clone(), self.mail_domain.clone()).expect("clone store error")
    }
}

impl Store {
    pub fn new(path: Option<String>, mail_domain: String) -> Result<Store> {
        let connection = if let Some(p) = &path {
            Connection::open(p)?
        } else {
            Connection::open_in_memory()?
        };
        let store = Store {
            connection,
            path,
            mail_domain,
        };
        store.init()?;
        Ok(store)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Store> {
        Store::new(None, "mail.xcf.io".to_string())
    }

    pub fn init(&self) -> Result<()> {
        if INITED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst) == Ok(false) {
            debug!("init store");
            self.connection.execute(
                r#"CREATE TABLE IF NOT EXISTS chat (
                        id VARCHAR(100) PRIMARY KEY
                    )"#,
                NO_PARAMS,
            )?;
        }
        Ok(())
    }

    pub fn add_bot_to_chat(&self, chat_id: &str) -> Result<()> {
        let affected = self
            .connection
            .execute("INSERT OR IGNORE INTO chat (id) VALUES (?)", &[chat_id])?;
        debug!("add bot to chat: {}, inserted: {}", chat_id, affected);
        Ok(())
    }

    pub fn remove_bot_from_chat(&self, chat_id: &str) -> Result<()> {
        let affected = self
            .connection
            .execute("DELETE FROM chat WHERE id = ?", &[chat_id])?;
        debug!("remove bot from chat: {}, affected: {}", chat_id, affected);
        Ok(())
    }

    pub fn exist_chat(&self, chat_id: &str) -> bool {
        debug!("exist chat: {}", chat_id);
        let count: isize = match self.connection.query_row(
            "SELECT count(0) FROM chat WHERE id = ?",
            &[chat_id],
            |row| row.get(0),
        ) {
            Ok(c) => c,

            Err(e) => {
                error!("query error: {}", e);
                0
            }
        };
        count > 0
    }

    pub fn mail_for_chat(&self, chat_id: &str) -> Result<String> {
        debug!("mail for chat: {}", chat_id);
        if !self.exist_chat(chat_id) {
            self.add_bot_to_chat(chat_id)?;
        }
        Ok(format!("{}@{}", chat_id, &self.mail_domain))
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
