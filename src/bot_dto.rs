use serde::{Deserialize, Serialize};

use crate::bot_server::feishu_client::MessageType;

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventRequest {
    Challenge(Challenge),
    EventV2(EventV2),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Challenge {
    challenge: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventV2 {
    pub schema: String,
    pub header: EventHeader,
    pub event: Event,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventHeader {
    pub event_id: String,
    pub event_type: String,
    pub create_time: String,
    pub token: String,
    pub app_id: String,
    pub tenant_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Event {
    ReceivedMessage(ReceivedMessage),
    AddOrRemoveBot(AddOrRemoveBot),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddOrRemoveBot {
    pub chat_id: String,
    pub operator_id: UserId,
    pub external: bool,
    pub operator_tenant_key: String,
    pub name: String,
    pub i18n_names: ChatI18nNames,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatI18nNames {
    en_us: String,
    zh_cn: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceivedMessage {
    pub sender: EventSender,
    pub message: EventMessage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventSender {
    pub sender_id: UserId,
    pub sender_type: String,
    pub tenant_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserId {
    pub union_id: String,
    pub user_id: String,
    pub open_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatType {
    #[serde(rename = "p2p")]
    P2p,
    Group,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventMessage {
    pub message_id: String,
    pub root_id: Option<String>,
    pub parent_id: Option<String>,
    pub create_time: String,
    pub update_time: String,
    pub chat_id: Option<String>,
    pub thread_id: Option<String>,
    pub chat_type: ChatType,
    pub message_type: MessageType,
    pub content: String,
    pub mentions: Vec<Mention>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Mention {
    pub key: String,
    pub id: UserId,
    pub name: String,
    pub tenant_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize() {
        let d = r#"
        {
    "schema": "2.0",
    "header": {
        "event_id": "5e3702a84e847582be8db7fb73283c02",
        "event_type": "im.message.receive_v1",
        "create_time": "1608725989000",
        "token": "rvaYgkND1GOiu5MM0E1rncYC6PLtF7JV",
        "app_id": "cli_9f5343c580712544",
        "tenant_key": "2ca1d211f64f6438"
    },
    "event": {
        "sender": {
            "sender_id": {
                "union_id": "on_8ed6aa67826108097d9ee143816345",
                "user_id": "e33ggbyz",
                "open_id": "ou_84aad35d084aa403a838cf73ee18467"
            },
            "sender_type": "user",
            "tenant_key": "736588c9260f175e"
        },
        "message": {
            "message_id": "om_5ce6d572455d361153b7cb51da133945",
            "root_id": "om_5ce6d572455d361153b7cb5xxfsdfsdfdsf",
            "parent_id": "om_5ce6d572455d361153b7cb5xxfsdfsdfdsf",
            "create_time": "1609073151345",
            "update_time": "1687343654666",
            "chat_id": "oc_5ce6d572455d361153b7xx51da133945",
            "thread_id": "omt_d4be107c616",
            "chat_type": "group",
            "message_type": "text",
            "content": "{\"text\":\"@_user_1 hello\"}",
            "mentions": [
                {
                    "key": "@_user_1",
                    "id": {
                        "union_id": "on_8ed6aa67826108097d9ee143816345",
                        "user_id": "e33ggbyz",
                        "open_id": "ou_84aad35d084aa403a838cf73ee18467"
                    },
                    "name": "Tom",
                    "tenant_key": "736588c9260f175e"
                }
            ],
            "user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_2_1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.4951.53 Safari/537.36 Lark/6.7.5 LarkLocale/en_US ttnet SDK-Version/6.7.8"
        }
    }
}
        "#;
        let d2 = r#"
        {"schema":"2.0","header":{"event_id":"28c07e7e9a1a875b994fd19f3784f227","token":"ONIdxMK2JZIKueTTVBspPcy5flH6XnBF","create_time":"1719389912680","event_type":"im.message.receive_v1","tenant_key":"2e7075328c8f165b","app_id":"cli_9ed975bb2df9900d"},"event":{"message":{"chat_id":"oc_3afec1ef7b7a16acacb15280078d4780","chat_type":"group","content":"{\"text\":\"@_user_1 a\"}","create_time":"1719389912273","mentions":[{"id":{"open_id":"ou_b6ae052c1064dee6c179d7497fd98c49","union_id":"on_a07d464680a65616946afb9ed8a177f7","user_id":""},"key":"@_user_1","name":"Mailhook","tenant_key":"2e7075328c8f165b"}],"message_id":"om_7d2a30e34eddaee786167d98f499f7f0","message_type":"text","update_time":"1719389912273"},"sender":{"sender_id":{"open_id":"ou_e5c903801bb5da8255f35d96a8dafc84","union_id":"on_e682501e6e0ea76a1d0ba6ae696f68e1","user_id":"g372d61a"},"sender_type":"user","tenant_key":"2e7075328c8f165b"}}}
        "#;
        let a: Result<EventRequest, serde_json::Error> = serde_json::from_str(&d);
        assert!(a.is_ok());
        let a2: Result<EventRequest, serde_json::Error> = serde_json::from_str(&d2);
        assert!(a2.is_ok());
    }
}
