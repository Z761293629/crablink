# TLS is terminated by a reverse proxy, which asks crablink before issuing a certificate

crablink serves plain HTTP and never handles certificates, even for custom Domains. The deployment docs put Caddy (on-demand TLS) in front of it. Before Caddy requests a certificate for a host, it calls crablink's internal `GET /internal/domain-allowed?domain=`, which only approves Domains an Instance Admin has approved, so nobody can point an arbitrary host at the instance and make it request certificates.

## Considered Options

- **Built-in ACME (e.g. `rustls-acme`)**: a truly single binary, but we would own certificate storage, renewal, rate limits and coordination between instances. Reconsider for the public service.
