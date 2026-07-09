# Rust login server — mirrors interlude_classic/login.Dockerfile.
# Build from the lineage2_rust directory:
#   docker build -f login.Dockerfile -t l2-rust-login .

FROM rust:1.96-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p loginserver

FROM alpine:3.20 AS runtime
RUN addgroup -S app_grp && adduser -S app -G app_grp
USER app
WORKDIR /home/app
# Same layout the binary expects: dist/login/config, dist/login/data, banned_ip.cfg.
ADD --chown=app:app_grp dist ./dist
COPY --chown=app:app_grp --from=builder /build/target/release/loginserver loginserver
EXPOSE 2106
EXPOSE 9014

# Config values are overridable via env: CONFIG_LOGINSERVER_<KEY>
# (e.g. CONFIG_LOGINSERVER_URL=jdbc:sqlite:./data/l2.db), matching the Java
# PropertiesParser env convention.
ENTRYPOINT ["./loginserver"]
