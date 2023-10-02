use super::query;
use super::Issue;
use crate::auth::{Role, RoleChecker, RoleGuard};
use async_graphql::{Context, InputObject, Object, Result};
use tokio::sync::mpsc;
use pyo3::types::{PyDict, PyModule};
use pyo3::{PyErr, Python};

#[derive(InputObject)]
pub struct UpdateIssue {
    assigned_to: Option<String>,
    description: Option<String>,
    enforce_down: Option<bool>,
    id: u32,
    title: Option<String>,
}
#[derive(InputObject)]
pub struct NewIssue {
    assigned_to: Option<String>,
    description: String,
    down_siblings: Option<bool>,
    enforce_down: Option<bool>,
    target: String,
    title: String,
}

impl NewIssue {
    async fn open(&self, operator: &str) -> u32 {
        let target = self.target.clone();
        let assigned_to = self.assigned_to.clone();
        let title = self.title.clone();
        let description = self.description.clone();
        let enforce_down = self.enforce_down.clone();
        let down_siblings = self.down_siblings.clone();
        let created_by = operator.to_string();
        tokio::task::spawn_blocking(move || {
            pyo3::prepare_freethreaded_python();
            Python::with_gil(|py| -> Result<u32, PyErr> {
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
                let kwargs = PyDict::new(py);
                let _ = kwargs.set_item("target", target);
                if let Some(a) = assigned_to {
                    let _ = kwargs.set_item("assigned_to", a);
                }
                let _ = kwargs.set_item("created_by", created_by);
                let _ = kwargs.set_item("title", title);
                let _ = kwargs.set_item("description", description);
                let _ = kwargs.set_item("severity", 3);
                if let Some(e) = enforce_down {
                    let _ = kwargs.set_item("enforce_down", e);
                } else {
                    let _ = kwargs.set_item("enforce_down", false);
                }
                if let Some(d) = down_siblings {
                    let _ = kwargs.set_item("down_siblings", d);
                } else {
                    let _ = kwargs.set_item("down_siblings", false);
                }
                let issue = ctt_module
                    .getattr("Issue")
                    .unwrap()
                    .call((), Some(kwargs))
                    .unwrap();
                let id = ctt.call_method1("open", (issue,)).unwrap();
                Ok(id.extract().unwrap())
            })
            .unwrap()
        })
        .await
        .unwrap()
    }
}

async fn issue_close(cttissue: u32, operator: String, comment: String) {
    tokio::task::spawn_blocking(move || {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| -> Result<(), PyErr> {
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
            let issue = ctt.call_method1("issue", (cttissue,)).unwrap();
            ctt.call_method1("close", (issue, operator, comment))
                .unwrap();
            Ok(())
        })
        .unwrap()
    })
    .await
    .unwrap()
}

pub struct Mutation;

#[Object]
impl Mutation {
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn open<'a>(&self, ctx: &Context<'a>, issue: NewIssue) -> Issue {
        //TODO get operator from authentication
        let usr = &ctx.data_opt::<RoleGuard>().unwrap().user;
        let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let _ = tx.send(format!("{}: Opening issue for {}: {}", usr, issue.target, issue.title)).await;
        query::issue_from_id(ctx, issue.open(usr).await)
            .await
            .unwrap()
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn close<'a>(&self, ctx: &Context<'a>, issue: u32, comment: String) -> String {
        let usr: String = ctx.data_opt::<RoleGuard>().unwrap().user.clone();
        let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let _ = tx.send(format!("{}: closing issue for {}: {}", usr, issue, comment)).await;
        issue_close(issue, usr, comment).await;
        "Closed".to_string()
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn update<'a>(&self, ctx: &Context<'a>, issue: UpdateIssue) -> Issue {
        tokio::task::spawn_blocking(move || {
            pyo3::prepare_freethreaded_python();
            Python::with_gil(|py| -> Result<(), PyErr> {
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
                let kwargs = PyDict::new(py);
                if let Some(a) = issue.assigned_to {
                    let _ = kwargs.set_item("assigned_to", a);
                }
                if let Some(a) = issue.description {
                    let _ = kwargs.set_item("description", a);
                }
                if let Some(a) = issue.enforce_down {
                    let _ = kwargs.set_item("enforce_down", a);
                }
                if let Some(a) = issue.title {
                    let _ = kwargs.set_item("title", a);
                }
                ctt.call_method("update", (issue.id, kwargs), None).unwrap();
                Ok(())
            })
            .unwrap()
        })
        .await
        .unwrap();
        query::issue_from_id(ctx, issue.id).await.unwrap()
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn drain<'a>(&self, ctx: &Context<'a>, issue: u32) -> String {
        let usr = &ctx.data_opt::<RoleGuard>().unwrap().user;
        let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let _ = tx.send(format!("{}: draing nodes for issue {}", usr, issue)).await;
        tokio::task::spawn_blocking(move || {
            pyo3::prepare_freethreaded_python();
            Python::with_gil(|py| -> Result<(), PyErr> {
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
                ctt.call_method1("prep_for_work", (issue, "todo")).unwrap();
                Ok(())
            })
            .unwrap()
        })
        .await
        .unwrap();
        "drained".to_string()
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn release<'a>(&self, ctx: &Context<'a>, issue: u32) -> String {
        let usr = &ctx.data_opt::<RoleGuard>().unwrap().user;
        let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let _ = tx.send(format!("{}: resuming nodes for issue {}", usr, issue)).await;
        tokio::task::spawn_blocking(move || {
            pyo3::prepare_freethreaded_python();
            Python::with_gil(|py| -> Result<(), PyErr> {
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
                ctt.call_method1("end_work", (issue, "todo")).unwrap();
                Ok(())
            })
            .unwrap()
        })
        .await
        .unwrap();
        "released".to_string()
    }
}
