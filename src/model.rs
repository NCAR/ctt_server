use async_graphql::{
    Context, Object, Result, Schema, EmptyMutation, EmptySubscription, Enum, ComplexObject, SimpleObject
};
use pyo3::{Python, PyErr};
use pyo3::types::PyModule;
use serde::{Serialize,Deserialize};
use chrono::NaiveDateTime;

pub type CttSchema = Schema<Query, EmptyMutation, EmptySubscription>;

#[derive(Serialize, Deserialize, SimpleObject)]
#[graphql(complex)]
pub struct Issue {
    id: u32,
    target: String,
    issue_status: IssueStatus,
    assigned_to: String,
    title: String,
    description: String,
    enforce_down: bool,
    down_siblings: bool,
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

#[ComplexObject]
impl Issue {
    async fn comments(&self) -> Vec<Comment> {
        let id = self.id.clone();
        tokio::task::spawn_blocking(move || {
            pyo3::prepare_freethreaded_python();
            Python::with_gil(|py| -> Result<Vec<Comment>, PyErr> {
                let ctt_module = PyModule::import(py, "ctt").unwrap();
                let conf = ctt_module.getattr("get_config").unwrap().call(("/home/shanks/projects/ctt/conf/ctt.ini","/home/s
    nks/projects/ctt/conf/secrets.ini",), None).unwrap();
                let ctt = ctt_module.getattr("CTT").unwrap().call((conf,), None).unwrap();
                let issue = ctt.call_method1("issue", (id,)).unwrap();
                let events = issue.getattr("comments").unwrap();
                let mut resp = Vec::new();
                for ev in events.iter().unwrap() {
                    let e = ev.unwrap();
                    let c = Comment {
                            author: e.getattr("created_by").unwrap().to_string(),
                            date: NaiveDateTime::parse_from_str(&e.getattr("created_at").unwrap().to_string(), "%Y-%m-%d %H:%M:%S").unwrap(),
                            comment:e.getattr("comment").unwrap().to_string(),
                    };
                    resp.push(c);
                }
                Ok(resp)
            }).unwrap()
        }).await.unwrap()
    }
}

async fn issue_from_id(_ctx: &Context<'_>, id: u32) -> Result<Issue, PyErr> {
    tokio::task::spawn_blocking(move || {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| -> Result<Issue, PyErr> {
            let ctt_module = PyModule::import(py, "ctt").unwrap();
            let conf = ctt_module.getattr("get_config").unwrap().call(("/home/shanks/projects/ctt/conf/ctt.ini","/home/s
nks/projects/ctt/conf/secrets.ini",), None).unwrap();
            let ctt = ctt_module.getattr("CTT").unwrap().call((conf,), None).unwrap();
            let issue = ctt.call_method1("issue", (id,)).unwrap();
            let issue_status = { if issue.getattr("status").unwrap().to_string() == "OPEN" 
                {IssueStatus::OPEN} else {IssueStatus::CLOSED}};
            Ok(Issue{
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
    }).await.unwrap()
}

async fn issues(_ctx: &Context<'_>) -> Result<Vec<Issue>, PyErr> {
    tokio::task::spawn_blocking(move || {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| -> Result<Vec<Issue>, PyErr> {
            let ctt_module = PyModule::import(py, "ctt").unwrap();
            let conf = ctt_module.getattr("get_config").unwrap().call(("/home/shanks/projects/ctt/conf/ctt.ini","/home/s
nks/projects/ctt/conf/secrets.ini",), None).unwrap();
            let ctt = ctt_module.getattr("CTT").unwrap().call((conf,), None).unwrap();
            let issues = ctt.call_method0("issue_list").unwrap();
            let mut resp = Vec::new();
            for i in issues.iter().unwrap() {
                let issue = i.unwrap();
                let issue_status = { if issue.getattr("status").unwrap().to_string() == "IssueStatus.OPEN" 
                    {IssueStatus::OPEN} else {IssueStatus::CLOSED}};
                resp.push(Issue{
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
    }).await.unwrap()
}

pub struct Query;

#[Object]
impl Query {
    async fn issue<'a>(
        &self,
        ctx: &Context<'a>,
        issue: u32
    ) -> Option<Issue> {
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
            issues = issues.into_iter().filter(|x| x.issue_status == status).collect()
        }
        if let Some(t) = target {
            issues = issues.into_iter().filter(|x| x.target == t).collect()
        }
        issues
    }
}
