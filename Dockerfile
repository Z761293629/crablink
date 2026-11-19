# Build the web UI, embed it in a static musl binary (ADR-0008), then ship that on plain Alpine:
# busybox wget does the health check, tini forwards SIGTERM (a bare Rust binary as PID 1 would
# ignore it).
FROM node:24-alpine AS web
# Keep in step with packageManager in web/package.json.
RUN npm install -g pnpm@12.21.0
WORKDIR /web
COPY web/package.json web/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm build

FROM rust:1.100-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
COPY --from=web /web/dist web/dist
RUN cargo build --release --locked -p crablink-server

# The DB-IP Lite country database (CC BY 4.0, no key needed) for Clicks' countries (ADR-0006).
# DB-IP publishes one a month; early in a month this month's may not be out yet, so fall back to last month's.
FROM alpine:3.22 AS country-db
RUN for month in "$(date +%Y-%m)" "$(date -d "@$(($(date +%s) - 15 * 86400))" +%Y-%m)"; do \
        wget -qO /dbip-country-lite.mmdb.gz "https://download.db-ip.com/free/dbip-country-lite-$month.mmdb.gz" && break; \
    done; \
    gunzip /dbip-country-lite.mmdb.gz

FROM alpine:3.22
RUN apk add --no-cache tini && adduser -D -H -u 10001 crablink
COPY --from=build /src/target/release/crablink-server /usr/local/bin/crablink-server
COPY --from=country-db /dbip-country-lite.mmdb /usr/share/crablink/dbip-country-lite.mmdb
USER crablink
ENV CRABLINK_LISTEN=0.0.0.0:8080
ENV CRABLINK_COUNTRY_DB=/usr/share/crablink/dbip-country-lite.mmdb
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s CMD wget -qO /dev/null http://127.0.0.1:8080/health || exit 1
ENTRYPOINT ["/sbin/tini", "--", "crablink-server"]
