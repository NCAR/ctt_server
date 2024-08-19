use crate::conf::Conf;
#[cfg(feature = "slack")]
use slack_morphism::{
    prelude::SlackApiChatPostMessageRequest, prelude::SlackClientHyperConnector, SlackApiToken,
    SlackApiTokenValue, SlackClient, SlackMessageContent,
};
#[cfg(feature = "slack")]
use std::collections::{BTreeMap, BTreeSet};
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
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
        operator: String,
    },
    Resume {
        target: String,
        operator: String,
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
    let mut interval = time::interval(Duration::from_secs(conf.poll_interval * 6));
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    let connector = SlackClientHyperConnector::new().unwrap();
    let client = SlackClient::new(connector);
    let token_value: SlackApiTokenValue = conf.slack.token.into();
    let token: SlackApiToken = SlackApiToken::new(token_value);
    //title: issues
    let mut close_issues: BTreeMap<String, BTreeSet<i32>> = BTreeMap::new();
    let mut update_issues: BTreeMap<String, BTreeSet<i32>> = BTreeMap::new();
    let mut open_issues: BTreeSet<i32> = BTreeSet::new();
    let mut operators: BTreeSet<String> = BTreeSet::new();
    let mut offline_nodes: BTreeSet<String> = BTreeSet::new();
    let mut resume_nodes: BTreeSet<String> = BTreeSet::new();

    loop {
        tokio::select! {
            Some(u) = rx.recv() => {
                match u {
                    ChangeLogMsg::Offline { target: t, operator: o } => {
                        offline_nodes.insert(t);
                        operators.insert(o);
                    }
                    ChangeLogMsg::Resume { target: t, operator: o } => {
                        resume_nodes.insert(t);
                        operators.insert(o);
                    }
                    ChangeLogMsg::Close {
                        issue: i,
                        title: t,
                        comment: _c,
                        operator: o,
                    } => {
                        if o != "ctt" {
                            if let Some(key) = close_issues.get_mut(&t) {
                                key.insert(i);
                            } else {
                                let mut tmp = BTreeSet::new();
                                tmp.insert(i);
                                close_issues.insert(t, tmp);
                            }
                            operators.insert(o);
                        }
                    }
                    ChangeLogMsg::Open {
                        issue: i,
                        title: _t,
                        operator: o,
                    } => {
                        if o != "ctt" {
                            open_issues.insert(i);
                            operators.insert(o);
                        }
                    }
                    ChangeLogMsg::Update {
                        issue: i,
                        operator: o,
                        title: t,
                    } => {
                        if let Some(key) = update_issues.get_mut(&t) {
                            key.insert(i);
                        } else {
                            let mut tmp = BTreeSet::new();
                            tmp.insert(i);
                            update_issues.insert(t, tmp);
                        }
                        operators.insert(o);
                    }
                }
            }
            _ = interval.tick() => {

                // don't care if its ctt doing anything besides offlining nodes (no operators and no
                // offline_nodes or if no nodes state is being changed (no resume_nodes or offline_nodes)
                if operators.is_empty() {
                    continue;
                }

                let session = client.open_session(&token);

                let mut msg  = format!("{:?}", operators);
                if !open_issues.is_empty() {
                    msg.push_str(&format!("\nOpened: {:?}", open_issues));
                }
                if !update_issues.is_empty() {
                    msg.push_str(&format!("\nUpdated: {:?}", update_issues));
                }
                if !close_issues.is_empty() {
                    msg.push_str(&format!("\nClosed: {:?}", close_issues));
                }
                if !offline_nodes.is_empty() {
                    msg.push_str(&format!("\nOfflined: {:?}", offline_nodes));
                }
                if !resume_nodes.is_empty() {
                    msg.push_str(&format!("\nResumed: {:?}", resume_nodes));
                }

                let post_chat_req = SlackApiChatPostMessageRequest::new(
                    format!("#{}", conf.slack.channel).into(),
                    SlackMessageContent::new().with_text(msg),
                );

                if let Err(e) = session.chat_post_message(&post_chat_req).await {
                    warn!("error sending slack message {}", e);
                };
                close_issues = BTreeMap::new();
                update_issues = BTreeMap::new();
                open_issues = BTreeSet::new();
                operators = BTreeSet::new();
                offline_nodes = BTreeSet::new();
                resume_nodes = BTreeSet::new();
            }
        }
    }
}
