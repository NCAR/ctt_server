use crate::conf::Conf;
#[cfg(feature = "slack")]
use slack_morphism::{
    prelude::SlackApiChatPostMessageRequest, prelude::SlackClientHyperConnector, SlackApiToken,
    SlackApiTokenValue, SlackClient, SlackMessageContent,
};
use tokio::sync::mpsc;
#[allow(unused_imports)]
use tracing::{info, instrument, warn, Level};

#[cfg(not(feature = "slack"))]
#[instrument]
pub async fn slack_updater(mut rx: mpsc::Receiver<String>, _conf: Conf) {
    let mut updates = vec![];
    while let Some(u) = rx.recv().await {
        updates.push(u);
    }
    if updates.is_empty() {
        return;
    }
    for m in updates {
        info!(m);
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum ChangeLogMsg {
    Offline {
        target: String,
    },
    Resume {
        target: String,
    },
    Close {
        issue: i32,
        title: String,
        comment: String,
        operator: String,
    },
    Open {
        issue: i32,
        title: String,
        operator: String,
    },
    Update {
        issue: i32,
        title: String,
        operator: String,
    },
}

#[cfg(feature = "slack")]
#[instrument(skip(conf))]
pub async fn slack_updater(mut rx: mpsc::Receiver<ChangeLogMsg>, conf: Conf) {
    use std::collections::{HashMap, HashSet};

    let connector = SlackClientHyperConnector::new().unwrap();
    let client = SlackClient::new(connector);
    let token_value: SlackApiTokenValue = conf.slack.token.into();
    let token: SlackApiToken = SlackApiToken::new(token_value);
    //title: issues
    let mut close_issues: HashMap<String, HashSet<i32>> = HashMap::new();
    let mut update_issues: HashMap<String, HashSet<i32>> = HashMap::new();
    let mut open_issues: HashSet<i32> = HashSet::new();
    let mut operators: HashSet<String> = HashSet::new();
    let mut offline_nodes: HashSet<String> = HashSet::new();
    let mut resume_nodes: HashSet<String> = HashSet::new();
    let mut comment: Option<String> = None;

    while let Some(u) = rx.recv().await {
        match u {
            ChangeLogMsg::Offline { target: t } => {
                offline_nodes.insert(t);
            }
            ChangeLogMsg::Resume { target: t } => {
                resume_nodes.insert(t);
            }
            ChangeLogMsg::Close {
                issue: i,
                title: t,
                comment: c,
                operator: o,
            } => {
                if o == "ctt" {
                    continue;
                }
                comment = Some(c);
                if let Some(key) = close_issues.get_mut(&t) {
                    key.insert(i);
                } else {
                    let mut tmp = HashSet::new();
                    tmp.insert(i);
                    close_issues.insert(t, tmp);
                }
                operators.insert(o);
            }
            ChangeLogMsg::Open {
                issue: i,
                title: t,
                operator: o,
            } => {
                if o == "ctt" {
                    continue;
                }
                comment = Some(t);
                open_issues.insert(i);
                operators.insert(o);
            }
            ChangeLogMsg::Update {
                issue: i,
                operator: o,
                title: t,
            } => {
                if let Some(key) = update_issues.get_mut(&t) {
                    key.insert(i);
                } else {
                    let mut tmp = HashSet::new();
                    tmp.insert(i);
                    update_issues.insert(t, tmp);
                }
                operators.insert(o);
            }
        }
    }

    // don't care if its ctt doing anything besides offlining nodes (no operators and no
    // offline_nodes or if no nodes state is being changed (no resume_nodes or offline_nodes)
    if (operators.is_empty() || resume_nodes.is_empty()) && offline_nodes.is_empty() {
        return;
    }

    let session = client.open_session(&token);

    let msg = if !open_issues.is_empty() {
        format!(
            "{:?} Opening issues: {:?},  '{}', Offlining {:?}",
            operators,
            open_issues,
            comment.unwrap(),
            offline_nodes,
        )
    } else if !update_issues.is_empty() {
        format!(
            "{:?} Updating issues: {:?}, Offlining: {:?}, Resuming: {:?}",
            operators, update_issues, offline_nodes, resume_nodes
        )
    } else if !close_issues.is_empty() {
        format!(
            "{:?} Closing issues: {:?}, '{}', Resuming {:?}",
            operators,
            close_issues,
            comment.unwrap(),
            resume_nodes,
        )
    } else {
        format!("ctt Offlined nodes: {:?}", offline_nodes)
    };
    let post_chat_req = SlackApiChatPostMessageRequest::new(
        format!("#{}", conf.slack.channel).into(),
        SlackMessageContent::new().with_text(msg),
    );

    if let Err(e) = session.chat_post_message(&post_chat_req).await {
        warn!("error sending slack message {}", e);
    };
}
