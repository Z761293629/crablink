# Build a static musl binary, then ship it on plain Alpine: busybox wget does the health check,
# tini forwards SIGTERM (a bare Rust binary as PID 1 would ignore it).
FROM rust:1.100-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p crablink-server

FROM alpine:3.22
RUN apk add --no-cache tini && adduser -D -H -u 10001 crablink
COPY --from=build /src/target/release/crablink-server /usr/local/bin/crablink-server
USER crablink
ENV CRABLINK_LISTEN=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s CMD wget -qO /dev/null http://127.0.0.1:8080/health || exit 1
ENTRYPOINT ["/sbin/tini", "--", "crablink-server"]
