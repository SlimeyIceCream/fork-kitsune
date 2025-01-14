use super::url::UrlService;
use crate::{
    error::{Error, Result},
    util::generate_secret,
};
use askama::Template;
use askama_axum::IntoResponse;
use axum::response::{Redirect, Response};
use chrono::Utc;
use diesel_async::RunQueryDsl;
use iso8601_timestamp::Timestamp;
use kitsune_db::{
    model::oauth2,
    schema::{oauth2_applications, oauth2_authorization_codes},
    PgPool,
};
use oxide_auth::endpoint::Scope;
use speedy_uuid::Uuid;
use std::str::{self, FromStr};
use strum::{AsRefStr, EnumIter, EnumMessage, EnumString};
use time::Duration;
use typed_builder::TypedBuilder;
use url::Url;

mod authorizer;
mod endpoint;
mod issuer;
mod registrar;
mod solicitor;

pub use self::{endpoint::OAuthEndpoint, solicitor::OAuthOwnerSolicitor};

/// If the Redirect URI is equal to this string, show the token instead of redirecting the user
const SHOW_TOKEN_URI: &str = "urn:ietf:wg:oauth:2.0:oob";
static AUTH_TOKEN_VALID_DURATION: Duration = Duration::minutes(10);

#[inline]
fn timestamp_to_chrono(ts: iso8601_timestamp::Timestamp) -> chrono::DateTime<Utc> {
    let secs = ts
        .duration_since(iso8601_timestamp::Timestamp::UNIX_EPOCH)
        .whole_seconds();
    chrono::NaiveDateTime::from_timestamp_opt(secs, ts.nanosecond())
        .unwrap()
        .and_utc()
}

#[inline]
fn chrono_to_timestamp(ts: chrono::DateTime<Utc>) -> iso8601_timestamp::Timestamp {
    time::OffsetDateTime::from_unix_timestamp(ts.timestamp())
        .unwrap()
        .replace_nanosecond(ts.timestamp_subsec_nanos())
        .unwrap()
        .into()
}

#[derive(AsRefStr, Clone, Copy, Debug, EnumIter, EnumMessage, EnumString)]
#[strum(serialize_all = "lowercase", use_phf)]
pub enum OAuthScope {
    #[strum(message = "Read admin-related data", serialize = "admin:read")]
    AdminRead,
    #[strum(message = "Write admin-related data", serialize = "admin:write")]
    AdminWrite,
    #[strum(message = "Read on your behalf")]
    Read,
    #[strum(message = "Write on your behalf")]
    Write,
}

#[derive(Clone, TypedBuilder)]
pub struct AuthorisationCode {
    application: oauth2::Application,
    scopes: Scope,
    state: Option<String>,
    user_id: Uuid,
}

#[derive(Clone, TypedBuilder)]
pub struct CreateApp {
    name: String,
    redirect_uris: String,
}

#[derive(Template)]
#[template(path = "oauth/token.html")]
struct ShowTokenPage {
    app_name: String,
    domain: String,
    token: String,
}

#[derive(Clone, TypedBuilder)]
pub struct OAuth2Service {
    db_conn: PgPool,
    url_service: UrlService,
}

impl OAuth2Service {
    pub async fn create_app(&self, create_app: CreateApp) -> Result<oauth2::Application> {
        let mut db_conn = self.db_conn.get().await?;

        diesel::insert_into(oauth2_applications::table)
            .values(oauth2::NewApplication {
                id: Uuid::now_v7(),
                secret: generate_secret().as_str(),
                name: create_app.name.as_str(),
                redirect_uri: create_app.redirect_uris.as_str(),
                scopes: "",
                website: None,
            })
            .get_result(&mut db_conn)
            .await
            .map_err(Error::from)
    }

    pub async fn create_authorisation_code_response(
        &self,
        AuthorisationCode {
            application,
            scopes,
            state,
            user_id,
        }: AuthorisationCode,
    ) -> Result<Response> {
        let mut db_conn = self.db_conn.get().await?;
        let authorization_code: oauth2::AuthorizationCode =
            diesel::insert_into(oauth2_authorization_codes::table)
                .values(oauth2::NewAuthorizationCode {
                    code: generate_secret().as_str(),
                    application_id: application.id,
                    user_id,
                    scopes: scopes.to_string().as_str(),
                    expires_at: Timestamp::now_utc() + AUTH_TOKEN_VALID_DURATION,
                })
                .get_result(&mut db_conn)
                .await?;

        if application.redirect_uri == SHOW_TOKEN_URI {
            Ok(ShowTokenPage {
                app_name: application.name,
                domain: self.url_service.domain().into(),
                token: authorization_code.code,
            }
            .into_response())
        } else {
            let mut url = Url::from_str(&application.redirect_uri)?;
            url.query_pairs_mut()
                .append_pair("code", &authorization_code.code);

            if let Some(state) = state {
                url.query_pairs_mut().append_pair("state", &state);
            }

            Ok(Redirect::to(url.as_str()).into_response())
        }
    }
}
