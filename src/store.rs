use anyhow::Result;
use log::{debug, error};
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::{Arc, Once};

pub struct Store {
    path: Option<String>,
    connection: Connection,
    mail_domain: String,
    inited: Arc<Once>,
}

impl Clone for Store {
    fn clone(&self) -> Self {
        let store = if let Some(p) = &self.path {
            let connection = Connection::open(p).unwrap();
            Store {
                connection,
                path: Some(p.clone()),
                mail_domain: self.mail_domain.clone(),
                inited: self.inited.clone(),
            }
        } else {
            let connection = Connection::open_in_memory().unwrap();
            Store {
                connection,
                path: None,
                mail_domain: self.mail_domain.clone(),
                inited: Arc::new(Once::new()),
            }
        };
        store.init();
        store
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
            inited: Arc::new(Once::new()),
        };
        store.init();
        Ok(store)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Store> {
        Store::new(None, "test".to_string())
    }

    fn init(&self) {
        self.inited.call_once(|| {
            self.init_raw().unwrap();
        });
    }

    fn init_raw(&self) -> Result<()> {
        debug!("init store");
        self.connection.execute(
            r#"CREATE TABLE IF NOT EXISTS chat (
                        id VARCHAR(100) PRIMARY KEY
                    )"#,
            (),
        )?;
        self.connection.execute(
            r#"CREATE TABLE IF NOT EXISTS mail (
                        id VARCHAR(100) PRIMARY KEY,
                        body BLOB
                    )"#,
            (),
        )?;
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

    pub fn save_mail(&self, id: &str, body: &Vec<u8>) -> Result<()> {
        let affected = self.connection.execute(
            "INSERT OR IGNORE INTO mail (id, body) VALUES (?, ?)",
            params![id, body],
        )?;
        debug!("save mail: {}, inserted: {}", id, affected);
        Ok(())
    }

    pub fn get_mail(&self, id: &str) -> Result<Option<Vec<u8>>> {
        debug!("get mail: {}", id);
        let body: Option<Vec<u8>> = self
            .connection
            .query_row("SELECT body FROM mail WHERE id = ?", &[id], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use crate::store::Store;

    #[test]
    fn test_add_or_remove_bot() {
        let store = Store::in_memory().unwrap();
        let chat_id = "some_chat_name";
        store.add_bot_to_chat(chat_id).unwrap();
        assert!(store.exist_chat(chat_id));
        store.remove_bot_from_chat(chat_id).unwrap();
        assert!(!store.exist_chat(chat_id))
    }

    #[test]
    fn test_save_and_store_mail() {
        let store = Store::in_memory().unwrap();
        let mail_id = "mail_id";
        let body = vec![0, 10, 20, 30, 40, 50, 100, 255, 123, 45, 2];
        store.save_mail(mail_id, &body).unwrap();
        assert_eq!(store.get_mail(mail_id).unwrap().unwrap(), body);
    }
}
