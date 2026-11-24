# The public service is an ordinary instance with its switches turned on

`crabl.ink` will run as a public service that anyone can sign up to, but it is the same binary that teams self-host, not a fork or a separate edition. Every capability the public service needs and a self-hosted team does not (open sign-up, email, rate limiting, safe-browsing checks, abuse handling) is an instance switch that is off by default, so a self-hosted instance behaves the same after an upgrade and `crabl.ink` is just an instance with those switches on.

## Considered Options

- **A separate public edition (fork or cargo feature)**: lets the public service move faster, but splits testing and releases, and leaves self-hosters without features they could have turned on themselves.
- **Public-service behaviour on by default**: simpler configuration for `crabl.ink`, but every self-hosted upgrade would open sign-up or start sending email unless its operator noticed and opted out.
