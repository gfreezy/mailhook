use anyhow::Result;
use melib::attachments::DecodeOptions;
use melib::Envelope;

pub struct MailContent {
    pub text: String,
    pub files: Vec<(String, Vec<u8>)>,
}

pub fn get_data_from_mail(mail: &[u8]) -> Result<MailContent> {
    let envelope = Envelope::from_bytes(mail, None)?;
    let attachment = envelope.body_bytes(mail);
    let body = attachment.text();
    let text = if let Some(sub) = envelope.subject {
        format!("{}\n{}", sub, body)
    } else {
        body
    };
    let mut files = Vec::new();
    for atta in attachment.attachments() {
        let Some(filename) = atta.filename() else {
            continue;
        };
        files.push((filename, atta.decode(DecodeOptions::default())));
    }
    Ok(MailContent { text, files })
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use melib::{Attachment, Envelope};

    #[test]
    fn test_parse_mail() {
        let content = r#"Received: by mail-ua1-f41.google.com with SMTP id o8so1970899uar.3
        for <oc_2799c1920a9c739f54bec782b90b6e78@mail.xcf.io>; Fri, 12 Mar 2021 08:50:44 -0800 (PST)
DKIM-Signature: v=1; a=rsa-sha256; c=relaxed/relaxed;
        d=gmail.com; s=20161025;
        h=mime-version:from:date:message-id:subject:to;
        bh=+IkSYOYTMVp9aYcrzpeOECvgu828+40cnqGLBnHa4b4=;
        b=RLJAVqelcPrN4tJ7Woq6jNJ1znfNcSH3P8eCj//7QYvOdkJud/pDeuKQTEaZ3Gyj5k
         otaYpzFmZlPnS9X6CvDo0IyXJO0+yvLmp06obielyY60LM7XK5ppBwFaZp13XJjFlG1f
         oMsapEAV/pP3hPnafFkEk3RqgwGi2i/G723dgJkSQxv6r3TN4/s+CRI7j3uK/k9FMXHK
         tqmOMaGcDuF6+ru2NusnW/9f/Cdo7ixAiZfAo5ZjqgGnM4fojhXZTsmm7GrD9RhGfkrC
         87sQZvVENOPj+a1naKTtf/bjOI/Pbxzk+8gdqDCtnxPNfpIZ7d8cd3yK10KWPcWERjTW
         2GCw==
X-Google-DKIM-Signature: v=1; a=rsa-sha256; c=relaxed/relaxed;
        d=1e100.net; s=20161025;
        h=x-gm-message-state:mime-version:from:date:message-id:subject:to;
        bh=+IkSYOYTMVp9aYcrzpeOECvgu828+40cnqGLBnHa4b4=;
        b=aKMu4lrJafdmd6+Sf1ChSwrqgQYMKY4wNNsIkfZmR+LPVkLcig2kz8fjPm9CiBcLOB
         Xv5SUsyEkNWtM6AM5ezH5ntHqSkvQV0u3nOF6q/EYP49APl8ZxIVaAUXEwd9S5XSW7y0
         tsiHSS647NFK1iS+Bqf2ZKG1INKE8MMbkv4y/fvTro1ZznPKZ//qWHMEsZbdul56GE7x
         A85aQIlcvhTEoXkfOU2f2QGDC63oFL9SbzA8JoCk4wM6kyVd6AVp1hdTdyLe/6Z0uUoB
         vAc42J6WCya6qK6CSBAcaOgxT9k9SMwZBMPqh7wavzkwvYqy2h004YUdncUr3eAaAucl
         bNvw==
X-Gm-Message-State: AOAM530qML0h8+qn+lja/WYg8jCpoKrK0NqE+WXGbGtY4LvpFD+ICx9a
	lx4H686HszRn5EQRhaHjBZ4lNZn26b2Ajfk09pitzQxmDn2DJg==
X-Google-Smtp-Source: ABdhPJzmT3GONNe+vr+4KFfZZK/gAW9HjZilRa4dWmMmjPBdqFIsrb7Su0K3rLX4YycrK1oaa88l6L8rwkXX4wX7O7k=
X-Received: by 2002:ab0:703a:: with SMTP id u26mr9313000ual.34.1615567842928;
 Fri, 12 Mar 2021 08:50:42 -0800 (PST)
MIME-Version: 1.0
From: Alex <gfreezy@gmail.com>
Date: Sat, 13 Mar 2021 00:50:31 +0800
Message-ID: <CAG=ro2ehv9NbVTPn20=PghZq-Tx9xa+RyCvXjHa0xMM4mhdoCA@mail.gmail.com>
Subject: test
To: oc_2799c1920a9c739f54bec782b90b6e78@mail.xcf.io
Content-Type: multipart/alternative; boundary="000000000000636b6205bd59b382"

--000000000000636b6205bd59b382
Content-Type: text/plain; charset="UTF-8"

bbb

--000000000000636b6205bd59b382
Content-Type: text/html; charset="UTF-8"

<div dir="ltr">bbb</div>

--000000000000636b6205bd59b382--
"#;
        let envelope = Envelope::from_bytes(content.as_bytes(), None).unwrap();
        expect![[r#"test"#]].assert_eq(envelope.subject().as_ref());
        expect![[r#"<CAG=ro2ehv9NbVTPn20=PghZq-Tx9xa+RyCvXjHa0xMM4mhdoCA@mail.gmail.com>"#]]
            .assert_eq(envelope.message_id_display().as_ref());

        let body = envelope.body_bytes(content.as_bytes());
        expect![[r#"multipart/alternative"#]].assert_eq(body.content_type().to_string().as_str());

        let body_text = body.text();
        expect![[r#"
            bbb
        "#]]
        .assert_eq(&body_text);

        let subattachments: Vec<Attachment> = body.attachments();
        assert_eq!(subattachments.len(), 3);
    }

    #[test]
    fn parse_gbk_mail() {
        let raw = r#"From: "=?gb18030?B?MW1pbjIwMTg=?=" <1min2018@xiachufang.com>
To: "=?gb18030?B?b2NfMjc5OWMxOTIwYTljNzM5ZjU0YmVjNzgyYjkwYjZlNzg=?=" <oc_2799c1920a9c739f54bec782b90b6e78@mail.xcf.io>
Subject: =?gb18030?B?zNrRtsbz0rXTys/k19S2r9eqt6LR6dak08q8/g==?=
Mime-Version: 1.0
Content-Type: multipart/alternative;
	boundary="----=_NextPart_6054AC6B_14AB2958_56E13774"
Content-Transfer-Encoding: 8Bit
Date: Fri, 19 Mar 2021 21:51:39 +0800
X-Priority: 3
Message-ID: <tencent_0610DD0B531A6A9C3F0F9AC5@qq.com>
X-QQ-MIME: TCMime 1.0 by Tencent
X-Mailer: QQMail 2.x
X-QQ-Mailer: QQMail 2.x
X-QQ-SENDSIZE: 520
Received: from qq.com (unknown [127.0.0.1])
	by smtp.qq.com (ESMTP) with SMTP
	id ; Fri, 19 Mar 2021 21:51:40 +0800 (CST)
Feedback-ID: default:xiachufang.com:qybgforeign:qybgforeign6
X-QQ-Bgrelay: 1

This is a multi-part message in MIME format.

"#;
        let envelope = Envelope::from_bytes(raw.as_bytes(), None).unwrap();
        expect![[r#"腾讯企业邮箱自动转发验证邮件"#]].assert_eq(envelope.subject().as_ref());
        expect![[r#"<tencent_0610DD0B531A6A9C3F0F9AC5@qq.com>"#]]
            .assert_eq(envelope.message_id_display().as_ref());

        let body = envelope.body_bytes(raw.as_bytes());
        expect![[r#"multipart/alternative"#]].assert_eq(body.content_type().to_string().as_str());

        let body_text = body.text();
        expect![[r#""#]].assert_eq(&body_text);
    }
}
