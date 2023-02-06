use futures_util::future;
use post_process::{Element, Html, Transformer};
use pretty_assertions::assert_eq;
use std::borrow::Cow;

#[tokio::test]
async fn link_transformation() {
    let text = "@真島@goro.org how are you doing? :friday-night: #龍が如く0";
    let transformer = Transformer::new(|elem| async move {
        let transformed = match elem {
            Element::Emote(emote) => Element::Html(Html {
                tag: Cow::Borrowed("a"),
                attributes: vec![(
                    Cow::Borrowed("href"),
                    Cow::Owned(format!("https://example.com/emote/{}", emote.content)),
                )],
                content: Box::new(Element::Emote(emote)),
            }),
            Element::Hashtag(hashtag) => Element::Html(Html {
                tag: Cow::Borrowed("a"),
                attributes: vec![(
                    Cow::Borrowed("href"),
                    Cow::Owned(format!("https://example.com/hashtag/{}", hashtag.content)),
                )],
                content: Box::new(Element::Hashtag(hashtag)),
            }),
            Element::Mention(mention) => Element::Html(Html {
                tag: Cow::Borrowed("a"),
                attributes: vec![(
                    Cow::Borrowed("href"),
                    Cow::Owned(format!(
                        "https://example.com/mention/{}/{}",
                        mention.username,
                        mention.domain.as_deref().unwrap_or_default()
                    )),
                )],
                content: Box::new(Element::Mention(mention)),
            }),
            elem => elem,
        };

        Ok(transformed)
    });

    insta::assert_snapshot!(transformer.transform(text).await.unwrap());
}

#[tokio::test]
async fn noop_transformation() {
    let text = "@真島@goro.org how are you doing? :friday-night: #龍が如く0";
    let transformer = Transformer::new(future::ok);
    let transformed = transformer.transform(text).await.unwrap();

    assert_eq!(text, transformed);
}
