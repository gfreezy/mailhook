use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    Text(TextMessage),
    Post(PostMessage),
    Image(ImageMessage),
    File(FileMessage),
}

impl Message {
    pub fn typ(&self) -> &'static str {
        match self {
            Message::Text(_) => "text",
            Message::Post(_) => "post",
            Message::Image(_) => "image",
            Message::File(_) => "file",
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Message::Text(TextMessage { text: text.into() })
    }

    pub fn image(image_key: impl Into<String>) -> Self {
        Message::Image(ImageMessage {
            image_key: image_key.into(),
        })
    }

    pub fn file(file_key: impl Into<String>) -> Self {
        Message::File(FileMessage {
            file_key: file_key.into(),
        })
    }

    pub fn post(zh_cn: PostLang) -> Self {
        Message::Post(PostMessage { zh_cn })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextMessage {
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostMessage {
    zh_cn: PostLang,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostLang {
    title: String,
    content: Vec<PostLine>,
}

impl PostLang {
    pub fn builder() -> PostLangBuilder {
        PostLangBuilder {
            title: String::new(),
            content: Vec::new(),
        }
    }
}

pub struct PostLangBuilder {
    title: String,
    content: Vec<PostLine>,
}

impl PostLangBuilder {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn new_line(self) -> PostLineBuilder {
        PostLineBuilder::new(self)
    }

    pub fn finish(self) -> PostLang {
        PostLang {
            title: self.title,
            content: self.content,
        }
    }
}

pub struct PostLineBuilder {
    line: PostLine,
    lang_builder: PostLangBuilder,
}

impl PostLineBuilder {
    fn new(lang: PostLangBuilder) -> Self {
        PostLineBuilder {
            line: Vec::new(),
            lang_builder: lang,
        }
    }

    pub fn text(mut self, text: impl Into<String>, un_escape: bool) -> Self {
        self.line.push(PostTag::Text(PostTagText {
            text: text.into(),
            un_escape,
        }));
        self
    }

    pub fn a(mut self, text: impl Into<String>, href: impl Into<String>) -> Self {
        self.line.push(PostTag::A(PostTagA {
            text: text.into(),
            href: href.into(),
        }));
        self
    }

    pub fn at(mut self, user_id: impl Into<String>, user_name: Option<String>) -> Self {
        self.line.push(PostTag::At(PostTagAt {
            user_id: user_id.into(),
            user_name,
        }));
        self
    }

    pub fn img(mut self, image_key: impl Into<String>, height: usize, width: usize) -> Self {
        self.line.push(PostTag::Img(PostTagImg {
            image_key: image_key.into(),
            width,
            height,
        }));
        self
    }

    pub fn new_line(mut self) -> Self {
        self.lang_builder.content.push(self.line);
        Self::new(self.lang_builder)
    }

    pub fn finish(mut self) -> PostLang {
        self.lang_builder.content.push(self.line);
        self.lang_builder.finish()
    }
}

type PostLine = Vec<PostTag>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "tag")]
#[serde(rename_all = "snake_case")]
enum PostTag {
    Text(PostTagText),
    A(PostTagA),
    At(PostTagAt),
    Img(PostTagImg),
}

#[derive(Debug, Serialize, Deserialize)]
struct PostTagText {
    text: String,
    un_escape: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct PostTagA {
    text: String,
    href: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PostTagAt {
    user_id: String,
    user_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PostTagImg {
    image_key: String,
    height: usize,
    width: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageMessage {
    image_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMessage {
    file_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[test]
    fn test_text_message() {
        let msg = TextMessage {
            text: "plain text message".to_string(),
        };

        expect![[r#"
            TextMessage {
                text: "plain text message",
            }
        "#]]
        .assert_debug_eq(&msg);
    }

    #[test]
    fn test_post_message() {
        let msg = PostLang::builder()
            .title("title")
            .new_line()
            .a("link", "href")
            .at("user_id", Some("user_name".to_string()))
            .img("image_key", 200, 300)
            .new_line()
            .text("text", false)
            .finish();
        expect![[r#"
            Ok(
                "{\"title\":\"title\",\"content\":[[{\"tag\":\"a\",\"text\":\"link\",\"href\":\"href\"},{\"tag\":\"at\",\"user_id\":\"user_id\",\"user_name\":\"user_name\"},{\"tag\":\"img\",\"image_key\":\"image_key\",\"height\":200,\"width\":300}],[{\"tag\":\"text\",\"text\":\"text\",\"un_escape\":false}]]}",
            )
        "#]].assert_debug_eq(&serde_json::to_string(&msg));
    }
}
