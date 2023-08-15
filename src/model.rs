use async_graphql::{
    Context, Object, Result, Schema, EmptyMutation, EmptySubscription, Enum, Union
};
use pyo3::{Python, PyErr};
use pyo3::types::PyModule;
use serde::{Serialize,Deserialize};
use chrono::NaiveDateTime;

pub type CttSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

#[derive(Serialize, Deserialize)]
pub struct Issue {
    id: u32,
    target: String,
    issue_status: IssueStatus,
}

#[derive(Serialize, Deserialize, Enum, Copy, Clone, Eq, PartialEq)]
pub enum IssueStatus {
    OPEN,
    CLOSED,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Comment {
    author: String,
    date: NaiveDateTime,
    comment: String,
}

#[Object]
impl Comment {
    async fn author(&self) -> &String {
        &self.author
    }

    async fn date(&self) -> String {
        let d = self.date.clone().to_string();
        d
    }

    async fn comment(&self) -> &String {
        &self.comment
    }
}


#[Object]
impl Issue {
    async fn id(&self) -> &u32 {
        &self.id
    }

    async fn target(&self) -> &String {
        &self.target
    }

    async fn issue_status(&self) -> &IssueStatus {
        &self.issue_status 
    }

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
                issue_status: issue_status
            })
        })
    }).await.unwrap()
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn issue<'a>(
        &self,
        ctx: &Context<'a>,
        issue: u32
    ) -> Option<Issue> {
        issue_from_id(ctx, issue).await.ok()
    }
}
