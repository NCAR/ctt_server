use async_graphql::{Context, Guard, Result};
use axum::body::BoxBody;
use axum::extract;
use axum::http::header;
use chrono::{NaiveDateTime, Utc};
use http::StatusCode;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tower_http::validate_request::ValidateRequest;
use tracing::info;
use users;

const SKETCHY_SECRET: &str = "6e313fae4b113e12c469edb558ccc92e331751efd5441c031802b04441efa7a3";

#[derive(Clone, Copy)]
pub struct Auth;

impl<B> ValidateRequest<B> for Auth {
    type ResponseBody = axum::body::BoxBody;

    fn validate(
        &mut self,
        request: &mut axum::http::Request<B>,
    ) -> axum::response::Result<(), axum::response::Response> {
        if let Some(user) = check_auth(&request) {
            // Set `user_id` as a request extension so it can be accessed by other
            // services down the stack.
            info!("Request validated for user {}", &user.user);
            request.extensions_mut().insert(user);

            Ok(())
        } else {
            let unauthorized_response: axum::response::Response =
                axum::response::Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(BoxBody::default())
                    .unwrap();
            info!("Invalid request");

            Err(unauthorized_response)
        }
    }
}

fn check_auth<B>(request: &axum::http::Request<B>) -> Option<RoleGuard> {
    info!("checking auth");
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|auth_header| auth_header.to_str().ok())
        .and_then(|auth_value| {
            if auth_value.starts_with("Bearer ") {
                Some(auth_value[7..].to_owned())
            } else {
                None
            }
        })
        .and_then(|t| {
            Some(
                decode::<RoleGuard>(
                    &t,
                    &DecodingKey::from_base64_secret(SKETCHY_SECRET).unwrap(),
                    &Validation::new(Algorithm::HS256),
                )
                .unwrap(),
            )
        })
        .and_then(|c| Some(c.claims))
}

#[derive(Deserialize, Debug)]
pub struct UserLogin {
    user: String,
    timestamp: NaiveDateTime,
}

#[derive(Serialize)]
pub struct Token {
    token: String,
}

async fn check_role(usr: &str) -> Option<Role> {
    let groups: HashSet<String> = users::get_user_by_name(usr)?
        .groups()?
        .iter()
        .map(|g| g.name().to_os_string().into_string())
        .filter_map(|x| x.ok())
        .collect();
    println!("{:?}", &groups);
    let admin = vec!["rdoot", "dhsg"];
    let guest = vec!["shanks"];
    for g in admin {
        if groups.contains(g) {
            info!("admin!");
            return Some(Role::Admin);
        }
    }
    for g in guest {
        if groups.contains(g) {
            info!("guest!");
            return Some(Role::Guest);
        }
    }
    None
}

pub async fn login_handler(
    extract::Json(payload): extract::Json<UserLogin>,
) -> Result<axum::Json<Token>, (StatusCode, String)> {
    info!("Login request: {:?}", payload);
    let role = check_role(&payload.user).await;
    if role.is_none() {
        info!("bad user");
        return Err((StatusCode::FORBIDDEN, "User not authorized".to_string()));
    }
    let role = role.unwrap();
    if payload.timestamp > Utc::now().naive_utc()
        || payload.timestamp < Utc::now().naive_utc() - chrono::Duration::minutes(2)
    {
        info!("bad timestamp");
        Err((StatusCode::BAD_REQUEST, "bad timestamp".to_string()))
    } else {
        let key = EncodingKey::from_base64_secret(SKETCHY_SECRET).unwrap();
        let claims = RoleGuard::new(
            role,
            payload.user,
            Utc::now().naive_utc() + chrono::Duration::minutes(6000),
        );
        let token = encode(&Header::default(), &claims, &key).unwrap();
        Ok(axum::Json(Token { token }))
    }
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
            exp: exp.timestamp() as usize,
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

#[async_trait::async_trait]
impl Guard for RoleChecker {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        if ctx.data_opt::<RoleGuard>().ok_or("no role")?.role == self.role {
            Ok(())
        } else {
            Err("Insufficient Permission".into())
        }
    }
}
