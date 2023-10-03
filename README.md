# ctt server
GraphQL api server for CTT

## Dev setup
- generate a cert with `openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -sha256 -days 3650 -nodes -subj "/C=XX/ST=StateName/L=CityName/O=CompanyName/OU=CompanySectionName/CN=127.0.0.1"`
- client needs cert

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
