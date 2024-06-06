use crate::conf::{Auth, Conf};
use async_graphql::{Context, Guard, Result};
use axum::body::Body;
use axum::extract;
use axum::http::header;
use axum::Extension;
use chrono::{NaiveDateTime, Utc};
use http::StatusCode;
#[allow(unused_imports)]
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use lazy_static::lazy_static;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tower_http::validate_request::ValidateRequest;
use tracing::{debug, info, warn};

lazy_static! {
    static ref SECRET: String = {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect::<String>()
    };
}

impl<B> ValidateRequest<B> for Auth {
    type ResponseBody = Body;

    fn validate(
        &mut self,
        request: &mut axum::http::Request<B>,
    ) -> axum::response::Result<(), axum::response::Response> {
        if let Some(user) = self.check_auth(request) {
            // Set `user_id` as a request extension so it can be accessed by other
            // services down the stack.
            info!("Request validated for user {}", &user.user);
            request.extensions_mut().insert(user);

            Ok(())
        } else {
            let unauthorized_response: axum::response::Response =
                axum::response::Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(Body::default())
                    .unwrap();
            info!("Invalid request");

            Err(unauthorized_response)
        }
    }
}

impl Auth {
    fn check_auth<B>(&self, request: &axum::http::Request<B>) -> Option<RoleGuard> {
        info!("checking auth");
        request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|auth_header| auth_header.to_str().ok())
            .and_then(|auth_value| {
                auth_value
                    .strip_prefix("Bearer ")
                    .map(|stripped| stripped.to_owned())
            })
            .map(|t| {
                decode::<RoleGuard>(
                    &t,
                    &DecodingKey::from_base64_secret(&SECRET).unwrap(),
                    &Validation::new(Algorithm::HS256),
                )
                .unwrap()
            })
            .map(|c| c.claims)
    }

    async fn check_role(&self, usr: &str, uid: u32) -> Option<Role> {
        let user = users::get_user_by_name(usr)?;
        if user.uid() != uid {
            debug!(
                "UID does not match expected user: {:?} expected uid: {}",
                usr, uid
            );
            return None;
        }
        let groups: HashSet<String> = user
            .groups()?
            .iter()
            .map(|g| g.name().to_os_string().into_string())
            .filter_map(|x| x.ok())
            .collect();
        for g in &self.admin {
            if groups.contains(g) {
                info!("admin!");
                return Some(Role::Admin);
            }
        }
        for g in &self.guest {
            if groups.contains(g) {
                info!("guest!");
                return Some(Role::Guest);
            }
        }
        None
    }
}
pub async fn login_handler(
    Extension(conf): Extension<Conf>,
    extract::Json(raw_payload): extract::Json<AuthRequest>,
) -> Result<axum::Json<Token>, (StatusCode, String)> {
    match raw_payload {
        // currently munge is the only supported auth method
        AuthRequest::Munge(payload) => {
            let payload = munge_auth::unmunge(payload.to_string());
            if let Err(e) = payload {
                warn!("unable to unmunge payload: {}", e);
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Unable to deserialize request".to_string(),
                ));
            }
            let payload: munge_auth::Message = payload.unwrap();
            info!("Login request: {:?}", payload);
            let uid = payload.uid;
            let payload = serde_json::from_str(&payload.msg);
            if payload.is_err() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Unable to deserialize request".to_string(),
                ));
            }
            let payload: UserLogin = payload.unwrap();
            info!("Login request: {:?}", payload);
            let role = conf.auth.check_role(&payload.user, uid).await;
            if role.is_none() {
                info!("bad user");
                return Err((StatusCode::FORBIDDEN, "User not authorized".to_string()));
            }
            let role = role.unwrap();
            let key = EncodingKey::from_base64_secret(&SECRET).unwrap();
            let claims = RoleGuard::new(
                role,
                // using payload.user is fine since its been authenticated via munge
                payload.user,
                Utc::now().naive_utc() + chrono::Duration::minutes(60),
            );
            let token = encode(&Header::default(), &claims, &key).unwrap();
            Ok(axum::Json(Token { token }))
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct UserLogin {
    user: String,
}

#[derive(Serialize)]
pub struct Token {
    token: String,
}

#[derive(Deserialize, Debug, Clone)]
pub enum AuthRequest {
    // munge encrypted Json<UserLogin>
    Munge(String),
}

#[derive(Eq, PartialEq, Copy, Clone, Serialize, Deserialize, Debug)]
pub enum Role {
    Admin,
    Guest,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoleGuard {
    role: Role,
    pub user: String,
    pub exp: usize,
}

impl RoleGuard {
    pub fn new(role: Role, user: String, exp: NaiveDateTime) -> Self {
        Self {
            role,
            user,
            exp: exp.and_utc().timestamp() as usize,
        }
    }
}

pub struct RoleChecker {
    role: Role,
}

impl RoleChecker {
    pub fn new(role: Role) -> Self {
        Self { role }
    }
}

impl Guard for RoleChecker {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        if ctx.data_opt::<RoleGuard>().ok_or("no role")?.role == self.role {
            Ok(())
        } else {
            Err("Insufficient Permission".into())
        }
    }
}
