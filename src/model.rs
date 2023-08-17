use async_graphql::{
    ComplexObject, Context, EmptySubscription, Enum, InputObject, Object, Result, Schema,
    SimpleObject,
};
use chrono::NaiveDateTime;
use pyo3::types::{PyDict, PyModule};
use pyo3::{PyErr, Python};
use serde::{Deserialize, Serialize};

pub type CttSchema = Schema<Query, Mutation, EmptySubscription>;

#[derive(Serialize, Deserialize, SimpleObject)]
#[graphql(complex)]
pub struct Issue {
    assigned_to: String,
    description: String,
    down_siblings: bool,
    enforce_down: bool,
    id: u32,
    issue_status: IssueStatus,
    target: String,
    title: String,
}

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
    assigned_to: String,
    description: String,
    down_siblings: bool,
    enforce_down: bool,
    target: String,
    title: String,
}

#[derive(Serialize, Deserialize, Enum, Copy, Clone, Eq, PartialEq)]
pub enum NodeStatus {
    ONLINE,
    DRAINING,
    DRAINED,
    OFFLINE,
    UNKNOWN,
}

#[derive(Serialize, Deserialize, Enum, Copy, Clone, Eq, PartialEq)]
pub enum IssueStatus {
    OPEN,
    CLOSED,
}

#[derive(Serialize, Deserialize, Clone, SimpleObject)]
pub struct Comment {
    author: String,
    date: NaiveDateTime,
    comment: String,
}

impl NewIssue {
    async fn open(&self, operator: String) -> u32 {
        let target = self.target.clone();
        let assigned_to = self.assigned_to.clone();
        let title = self.title.clone();
        let description = self.description.clone();
        let enforce_down = self.enforce_down.clone();
        let created_by = operator.clone();
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
                let kwargs = PyDict::new(py);
                kwargs.set_item("target", target);
                kwargs.set_item("assigned_to", assigned_to);
                kwargs.set_item("created_by", created_by);
                kwargs.set_item("title", title);
                kwargs.set_item("description", description);
                kwargs.set_item("enforce_down", enforce_down);
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

#[ComplexObject]
impl Issue {
    async fn comments(&self) -> Vec<Comment> {
        let id = self.id.clone();
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
    }
}

async fn issue_from_id(_ctx: &Context<'_>, id: u32) -> Result<Issue, PyErr> {
    tokio::task::spawn_blocking(move || {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| -> Result<Issue, PyErr> {
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
            let issue_status = {
                if issue.getattr("status").unwrap().to_string() == "OPEN" {
                    IssueStatus::OPEN
                } else {
                    IssueStatus::CLOSED
                }
            };
            Ok(Issue {
                id: issue.getattr("id").unwrap().extract().unwrap(),
                target: issue.getattr("target").unwrap().to_string(),
                issue_status: issue_status,
                assigned_to: issue.getattr("assigned_to").unwrap().to_string(),
                title: issue.getattr("title").unwrap().to_string(),
                description: issue.getattr("description").unwrap().to_string(),
                enforce_down: issue.getattr("enforce_down").unwrap().extract().unwrap(),
                down_siblings: issue.getattr("down_siblings").unwrap().extract().unwrap(),
            })
        })
    })
    .await
    .unwrap()
}

async fn issues(_ctx: &Context<'_>) -> Result<Vec<Issue>, PyErr> {
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
            let issues = ctt.call_method0("issue_list").unwrap();
            let mut resp = Vec::new();
            for i in issues.iter().unwrap() {
                let issue = i.unwrap();
                let issue_status = {
                    if issue.getattr("status").unwrap().to_string() == "IssueStatus.OPEN" {
                        IssueStatus::OPEN
                    } else {
                        IssueStatus::CLOSED
                    }
                };
                resp.push(Issue {
                    id: issue.getattr("id").unwrap().extract().unwrap(),
                    target: issue.getattr("target").unwrap().to_string(),
                    issue_status: issue_status,
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
}

pub struct Query;

#[Object]
impl Query {
    async fn issue<'a>(&self, ctx: &Context<'a>, issue: u32) -> Option<Issue> {
        issue_from_id(ctx, issue).await.ok()
    }

    async fn issues<'a>(
        &self,
        ctx: &Context<'a>,
        issue_status: Option<IssueStatus>,
        target: Option<String>,
    ) -> Vec<Issue> {
        let mut issues = issues(ctx).await.unwrap();
        if let Some(status) = issue_status {
            issues = issues
                .into_iter()
                .filter(|x| x.issue_status == status)
                .collect()
        }
        if let Some(t) = target {
            issues = issues.into_iter().filter(|x| x.target == t).collect()
        }
        issues
    }
}

pub struct Mutation;

#[Object]
impl Mutation {
    async fn open<'a>(&self, ctx: &Context<'a>, issue: NewIssue) -> Issue {
        //TODO get operator from authentication
        issue_from_id(ctx, issue.open("todo".to_string()).await)
            .await
            .unwrap()
    }
    async fn close<'a>(&self, issue: u32, comment: String) -> String {
        //TODO get operator from authentication
        issue_close(issue, "todo".to_string(), comment).await;
        "Closed".to_string()
    }
    async fn update<'a>(&self, ctx: &Context<'a>, issue: UpdateIssue) -> Issue {
        //TODO get operator from authentication
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
                    kwargs.set_item("assigned_to", a);
                }
                if let Some(a) = issue.description {
                    kwargs.set_item("description", a);
                }
                if let Some(a) = issue.enforce_down {
                    kwargs.set_item("enforce_down", a);
                }
                if let Some(a) = issue.title {
                    kwargs.set_item("title", a);
                }
                ctt.call_method("update", (issue.id, kwargs), None).unwrap();
                Ok(())
            })
            .unwrap()
        })
        .await
        .unwrap();
        issue_from_id(ctx, issue.id).await.unwrap()
    }
    async fn drain(&self, issue: u32) -> String {
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
    async fn release(&self, issue: u32) -> String {
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
