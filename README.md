# ctt server
GraphQL api server for CTT

## Features
- `pbs`, `slack`, and `auth` are all default features
- `pbs` enables interaction with the pbs job scheduler
- `slack` enables sending slack messages on certain events
- `auth` enables authentication, using posix groups on the server node
  - currently the only flow uses munge, however other flows planned (eventually...)

## Dev setup
- generate a cert with `openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -sha256 -days 3650 -nodes -subj "/C=XX/ST=StateName/L=CityName/O=CompanyName/OU=CompanySectionName/CN=127.0.0.1"`
- client needs cert
- `cargo run --no-default-features`

## querys
```
mutation OpenIssue($newIssue: NewIssue!) {
  open(issue: $newIssue) {
    id,
    target{name,status}
  }
}

{
  "newIssue": {
    "title": "test ticket",
    "description": "a test ticket description",
    "target": "tn0002"
  }
}
```

```
mutation CloseIssue($id: Int!, $comment: String!) {
  close(issue: $id, comment: $comment)
}

{
  "id": 1,
  "comment": "closing ticket"
}
```

```
mutation UpdateIssue($issue: UpdateIssue!) {
  updateIssue(issue: $issue){
    title,
    id,
    assignedTo,
    description,
    toOffline,
    enforceDown,
  }
}

{
  "issue": {
    "id": 1,
    "assignedTo": "fred",
    "description": "a new description",
    "enforceDown": true,
    "toOffline": "SIBLINGS",
    "title": "changed title"
  }
}
```

```
query ListIssues($status: IssueStatus, $target: String) {
  issues(issueStatus: $status, target: $target) {
    id,
    title,
    assignedTo,
    description,
    toOffline,
    target{name, status},
  }
}

{
  "status": "OPEN"
}
```

```
query GetIssue($id: Int!){
  issue(issue: $id){
    assignedTo,
    createdAt,
    createdBy,
    description,
    toOffline,
    enforceDown,
    id,
    issueStatus,
    title,
    comments{createdBy, comment, createdAt},
    target{name, status}
  }
}

{
  "id": 1
}
```
