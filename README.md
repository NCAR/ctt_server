# ctt server
GraphQL api server for CTT

## Notes
- uses [cargo-generate-rpm](https://crates.io/crates/cargo-generate-rpm) to build rpms
- currently the only auth flow uses munge, however other flows planned (eventually...)
- there are currently 2 scheduler implementations
  - shell -- shells out for each cmd
  - pbs -- links to pbs headers (pbs feature required) and comunicates with the server directly
- there are currently 2 cluster implementations
  - shell -- shells out for each cmd
  - regex -- you can describe you cluster with a few options and it'll handle the rest
- see `conf_ex.yaml` and `src/conf.rs` for an example config and what valid config options are

## Dev setup
- generate a cert with `openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -sha256 -days 3650 -nodes -subj "/C=XX/ST=StateName/L=CityName/O=CompanyName/OU=CompanySectionName/CN=127.0.0.1"`
- client needs cert
- `cargo run --no-default-features`
  - disables auth (assumes all requests come from an admin) and pbs (don't need to link to it)
- can explore graphiQL playground in your browser at the server_addr in your config (https)
  - can get schema as text from `https://$server_addr/api/schema`
