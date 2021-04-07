pub mod message;

use actix_web::web::block;
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

use self::message::Message;

#[derive(Clone)]
pub struct Client {
    app_id: String,
    app_secret: String,
}

#[derive(Serialize, Deserialize)]
struct SendTextMessageRequest {
    chat_id: String,
    root_id: Option<String>,
    msg_type: String,
    content: Content,
}

#[derive(Serialize, Deserialize)]
struct Content {
    text: String,
}

#[derive(Debug)]
pub enum ReceiverId {
    OpenId(String),
    UserId(String),
    UnionId(String),
    Email(String),
    ChatId(String),
}

impl ReceiverId {
    pub fn id(&self) -> &str {
        match self {
            ReceiverId::OpenId(s) => s,
            ReceiverId::UserId(s) => s,
            ReceiverId::UnionId(s) => s,
            ReceiverId::Email(s) => s,
            ReceiverId::ChatId(s) => s,
        }
    }

    pub fn typ(&self) -> &str {
        match self {
            ReceiverId::OpenId(_) => "open_id",
            ReceiverId::UserId(_) => "user_id",
            ReceiverId::UnionId(_) => "union_id",
            ReceiverId::Email(_) => "email",
            ReceiverId::ChatId(_) => "chat_id",
        }
    }
}

impl Client {
    pub fn new(app_id: String, app_secret: String) -> Self {
        Client { app_id, app_secret }
    }

    pub fn reply_text_message(
        &self,
        chat_id: String,
        text: String,
        root_id: Option<String>,
    ) -> Result<()> {
        #[derive(Serialize, Deserialize)]
        struct Resp {
            code: usize,
            msg: String,
        }

        let req = SendTextMessageRequest {
            chat_id,
            root_id,
            msg_type: "text".to_string(),
            content: Content { text },
        };
        let token = self.get_tenant_access_token();
        let resp: Resp = ureq::post("https://open.feishu.cn/open-apis/message/v4/send/")
            .set("Authorization", &format!("Bearer {}", token?))
            .send_json(serde_json::to_value(req)?)?
            .into_json()?;
        ensure!(resp.code == 0, resp.msg);
        Ok(())
    }

    pub fn send_message(&self, receiver_id: &ReceiverId, message: &Message) -> Result<()> {
        #[derive(Serialize, Deserialize)]
        struct Resp {
            code: usize,
            msg: String,
        }

        #[derive(Serialize, Deserialize)]
        struct Req {
            receive_id: String,
            content: String,
            msg_type: String,
        }

        let content = serde_json::to_string(message)?;
        let req = Req {
            receive_id: receiver_id.id().to_string(),
            content,
            msg_type: message.typ().to_string(),
        };
        let token = self.get_tenant_access_token();
        let resp: Resp = ureq::post("https://open.feishu.cn/open-apis/im/v1/messages")
            .set("Authorization", &format!("Bearer {}", token?))
            .query("receive_id_type", receiver_id.typ())
            .send_json(serde_json::to_value(req)?)?
            .into_json()?;
        ensure!(resp.code == 0, resp.msg);
        Ok(())
    }

    pub async fn reply_text_message_async(
        &self,
        chat_id: String,
        text: String,
        root_id: Option<String>,
    ) -> Result<()> {
        let self_clone = self.clone();
        let _ = block(move || self_clone.reply_text_message(chat_id, text, root_id)).await?;
        Ok(())
    }

    pub async fn send_text_message_async(&self, chat_id: String, text: String) -> Result<()> {
        self.reply_text_message_async(chat_id, text, None).await
    }

    pub fn get_tenant_access_token(&self) -> Result<String> {
        #[derive(Serialize, Deserialize)]
        struct Resp {
            code: isize,
            msg: String,
            tenant_access_token: String,
            expire: usize,
        }

        let resp: Resp =
            ureq::post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal/")
                .send_json(ureq::json! ({
                    "app_id": self.app_id,
                    "app_secret": self.app_secret
                }))?
                .into_json()?;
        ensure!(resp.code == 0, resp.msg);
        Ok(resp.tenant_access_token)
    }
}
