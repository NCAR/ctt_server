use sea_orm::{DatabaseConnection, ColumnTrait, EntityTrait, QueryFilter};
use crate::auth::{Role, RoleChecker};
use async_graphql::{ComplexObject, Context, Enum, Object, Result, SimpleObject};
use chrono::NaiveDateTime;
use pyo3::types::PyModule;
use pyo3::{PyErr, Python};
use serde::{Deserialize, Serialize};
use tracing::log::info;
use crate::entities::issue::IssueStatus;
use crate::entities::prelude::*;

#[derive(Serialize, Deserialize, Enum, Copy, Clone, Eq, PartialEq)]
pub enum NodeStatus {
    Online,
    Draining,
    Drained,
    Offline,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, SimpleObject)]
pub struct Comment {
    author: String,
    date: NaiveDateTime,
    comment: String,
}

#[ComplexObject]
impl Issue {
    async fn comments(&self) -> Vec<Comment> {
        todo!()
        /*
        let id = self.id;
        tokio::task::spawn_blocking(move || {
            pyo3::prepare_freethreaded_python();
            Python::with_gil(|py| -> Result<Vec<Comment>, PyErr> {
                let ctt_module = PyModule::import(py, "ctt").unwrap();
                let conf = ctt_module
                    .getattr("get_config")
                    .unwrap()
                    .call(
                        (
                            "/home/shanks/projects/ctt/conf/ctt.ini",
                            "/home/s
    nks/projects/ctt/conf/secrets.ini",
                        ),
                        None,
                    )
                    .unwrap();
                let ctt = ctt_module
                    .getattr("CTT")
                    .unwrap()
                    .call((conf,), None)
                    .unwrap();
                let issue = ctt.call_method1("issue", (id,)).unwrap();
                let events = issue.getattr("comments").unwrap();
                let mut resp = Vec::new();
                for ev in events.iter().unwrap() {
                    let e = ev.unwrap();
                    let c = Comment {
                        author: e.getattr("created_by").unwrap().to_string(),
                        date: NaiveDateTime::parse_from_str(
                            &e.getattr("created_at").unwrap().to_string(),
                            "%Y-%m-%d %H:%M:%S",
                        )
                        .unwrap(),
                        comment: e.getattr("comment").unwrap().to_string(),
                    };
                    resp.push(c);
                }
                Ok(resp)
            })
            .unwrap()
        })
        .await
        .unwrap()
    */
    }
}

async fn issues(_ctx: &Context<'_>) -> Result<Vec<Issue>, PyErr> {
    todo!()
    /*
    info!("issues func");
    tokio::task::spawn_blocking(move || {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| -> Result<Vec<Issue>, PyErr> {
            let ctt_module = PyModule::import(py, "ctt").unwrap();
            let conf = ctt_module
                .getattr("get_config")
                .unwrap()
                .call(
                    (
                        "/home/shanks/projects/ctt/conf/ctt.ini",
                        "/home/shanks/projects/ctt/conf/secrets.ini",
                    ),
                    None,
                )
                .unwrap();
            let ctt = ctt_module
                .getattr("CTT")
                .unwrap()
                .call((conf,), None)
                .unwrap();
            let issues = ctt.call_method0("issue_list").unwrap();
            let mut resp = Vec::new();
            info!("foobar");
            for i in issues.iter().unwrap() {
                info!("{:?}", i);
                let issue = i.unwrap();
                let issue_status = {
                    if issue.getattr("status").unwrap().to_string() == "IssueStatus.Open" {
                        IssueStatus::Open
                    } else {
                        IssueStatus::Closed
                    }
                };
                resp.push(Issue {
                    id: issue.getattr("id").unwrap().extract().unwrap(),
                    target: issue.getattr("target").unwrap().to_string(),
                    issue_status,
                    assigned_to: issue.getattr("assigned_to").unwrap().to_string(),
                    title: issue.getattr("title").unwrap().to_string(),
                    description: issue.getattr("description").unwrap().to_string(),
                    enforce_down: issue.getattr("enforce_down").unwrap().extract().unwrap(),
                    down_siblings: issue.getattr("down_siblings").unwrap().extract().unwrap(),
                });
            }
            Ok(resp)
        })
    })
    .await
    .unwrap()
    */
}

pub struct Query;

#[Object]
impl Query {
    #[graphql(guard = "RoleChecker::new(Role::Admin).or(RoleChecker::new(Role::Guest))")]
    async fn issue<'a>(&self, ctx: &Context<'a>, issue: i32) -> Option<crate::entities::issue::Model> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        crate::entities::prelude::Issue::find_by_id(issue)
            .one(db).await.unwrap()
    }

    #[graphql(guard = "RoleChecker::new(Role::Admin).or(RoleChecker::new(Role::Guest))")]
    async fn issues<'a>(
        &self,
        ctx: &Context<'a>,
        issue_status: Option<crate::entities::issue::IssueStatus>,
        target: Option<String>,
    ) -> Vec<crate::entities::issue::Model> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let mut select = crate::entities::prelude::Issue::find();
        if let Some(status) =  issue_status {
            select = select.filter(crate::entities::issue::Column::IssueStatus.eq(status));
        }
        if let Some(t) = target {
            select = select.filter(crate::entities::issue::Column::Target.eq(t));
        }
        select.all(db).await.unwrap()
    }
}
