pub(crate) mod feishu_client;

use crate::bot_server::feishu_client::Client;
use crate::store::Store;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use anyhow::Result;
use log::{debug, trace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Challenge {
    challenge: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EventHeader {
    event_id: String,
    token: String,
    create_time: String,
    event_type: String,
    tenant_key: String,
    app_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Event {
    TextMessage(TextMessage),
    AddOrRemoveBot(AddOrRemoveBot),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum EventRequest {
    Challenge(Challenge),
    EventV1 {
        ts: String,
        uuid: String,
        token: String,
        #[serde(rename = "type")]
        type_: String,
        event: Event,
    },
    EventV2 {
        schema: String,
        header: EventHeader,
        event: Event,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextMessage {
    #[serde(rename = "type")]
    type_: String,
    app_id: String,
    tenant_key: String,
    root_id: Option<String>,
    parent_id: String,
    open_chat_id: String,
    chat_type: String,
    msg_type: String,
    open_id: String,
    employee_id: String,
    union_id: String,
    open_message_id: String,
    is_mention: bool,
    text: String,
    text_without_at_bot: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddOrRemoveBot {
    app_id: String,
    chat_i18n_names: ChatI18nNames,
    chat_name: String,
    chat_owner_employee_id: String,
    chat_owner_name: String,
    chat_owner_open_id: String,
    open_chat_id: String,
    operator_employee_id: String,
    operator_name: String,
    operator_open_id: String,
    owner_is_bot: bool,
    tenant_key: String,
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatI18nNames {
    en_us: String,
    zh_cn: String,
}

async fn event(
    req: web::Json<EventRequest>,
    store: web::Data<Store>,
    client: web::Data<Client>,
) -> HttpResponse {
    trace!("event: {:?}", &req);
    let event = match &*req {
        EventRequest::Challenge(c) => return HttpResponse::Ok().json(c),
        EventRequest::EventV1 { event, .. } => event,
        EventRequest::EventV2 { event, .. } => event,
    };
    let ret = match event {
        Event::AddOrRemoveBot(e) => on_add_or_remove_bot(&store, &client, e).await,
        Event::TextMessage(e) => on_text_message(&store, &client, e).await,
    };
    if let Err(e) = ret {
        return HttpResponse::InternalServerError().json(e.to_string());
    }

    return HttpResponse::Ok().json("ok");
}

async fn on_add_or_remove_bot(store: &Store, client: &Client, msg: &AddOrRemoveBot) -> Result<()> {
    let open_chat_id = msg.open_chat_id.clone();
    match msg.type_.as_str() {
        "add_bot" => {
            store.add_bot_to_chat(&open_chat_id)?;
            let mail = store.mail_for_chat(&open_chat_id)?;
            let text = format!("Email address: {}", mail);
            let _ = client.send_text_message_async(open_chat_id, text).await;
        }
        "remove_bot" => store.remove_bot_from_chat(&open_chat_id)?,
        _ => unreachable!(),
    };
    Ok(())
}

async fn on_text_message(store: &Store, client: &Client, msg: &TextMessage) -> Result<()> {
    debug!("on text message");
    let text = match msg.chat_type.as_str() {
        "private" => "请在群组中@我".to_string(),
        "group" => store.mail_for_chat(&msg.open_chat_id)?,
        _ => unreachable!(),
    };

    client
        .reply_text_message_async(
            msg.open_chat_id.clone(),
            text,
            Some(msg.open_message_id.clone()),
        )
        .await?;
    Ok(())
}

async fn challenge(req: web::Json<Challenge>) -> impl Responder {
    req
}

async fn index() -> impl Responder {
    "hello"
}

#[actix_web::main]
pub(crate) async fn serve(client: Client, store: Store) -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .data(client.clone())
            .data(store.clone())
            .route("/challenge", web::post().to(challenge))
            .route("/event", web::post().to(event))
            .route("/", web::get().to(index))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
