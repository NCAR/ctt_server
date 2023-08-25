# ctt server
GraphQL api server for CTT


## querys
- list issues
```
{
  issues(issueStatus: OPEN, target: "gu0008") {
    id
    target
    assignedTo
    title
    issueStatus
    comments {
      comment
    }
  }
}
```

- open issue
```
mutation open($newIssue: NewIssue!) {
  open(issue: $newIssue) {
    id
  }
}

{
  "newIssue": {
    "target": "gu0008",
    "assignedTo": "shanks",
    "title": "graphql test2",
    "description": "making issue via graphql api",
    "enforceDown": false,
    "createdBy": "shanks"
  }
}
```

- close issue
```
mutation close($id: Int!, $comment: String!) {
  close(issue: $id, comment: $comment)
}

{
  "id": 1,
  "comment": "closing via api"
}
```
