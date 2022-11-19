use crate::{
    db::entity::user,
    error::{Error, Result},
};
use axum::http::Uri;
use phenomenon_ap::object::Note;
use phenomenon_http_signatures::Request;
use reqwest::Client;
use rsa::pkcs8::{self, SecretDocument};
use sha2::{Digest, Sha256};

/// Delivers ActivityPub activities
///
/// Does not need to be Arc wrapped for cheap cloning. It's inherently cheap to clone.
#[derive(Clone)]
pub struct Deliverer {
    client: Client,
}

impl Deliverer {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(concat!(
                    env!("CARGO_PKG_NAME"),
                    "/",
                    env!("CARGO_PKG_VERSION"),
                ))
                .build()
                .unwrap(),
        }
    }

    pub async fn deliver(&self, inbox_url: &str, user: &user::Model, note: &Note) -> Result<()> {
        let (_label, private_key) =
            SecretDocument::from_pem(user.private_key.as_ref().ok_or(Error::BrokenRecord)?)
                .map_err(pkcs8::Error::from)?;
        let body = serde_json::to_string(&note)?;
        let body_digest = base64::encode(Sha256::digest(body.as_bytes()));
        let digest_header = format!("sha-256={body_digest}");

        let mut request = self
            .client
            .get(inbox_url)
            .header("Digest", &digest_header)
            .body(body.clone())
            .build()?;

        let key_id = format!("{}#main-key", user.url);
        let headers = request.headers().clone();
        let uri =
            Uri::try_from(request.url().as_str()).expect("[Bug] Invalid URI stored in url::Url");
        let method = request.method().clone();
        let private_key = private_key.clone();

        let signature_header = crate::blocking::cpu(move || {
            let request = Request {
                headers: &headers,
                uri: &uri,
                method: &method,
            };

            phenomenon_http_signatures::sign(
                request,
                &["(request-target)", "digest", "date"],
                &key_id,
                private_key.as_bytes(),
            )
        })
        .await??;

        request.headers_mut().insert("Signature", signature_header);
        if !self.client.execute(request).await?.status().is_success() {
            todo!("return error");
        }

        Ok(())
    }
}
