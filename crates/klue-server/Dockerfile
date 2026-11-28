# Klue Server. Built from the repository's root, since it shares crates with the app:
#
#   docker build -f crates/klue-server/Dockerfile -t klue-server .
#   docker run -d --name klue-server --restart unless-stopped -p 8443:8443 -v klue-data:/data klue-server
#   docker logs klue-server
#
# TZ sets when the daily report goes out; Klue's carriers are Egyptian, so Cairo by default.

FROM rust:1-bookworm AS build
# BoringSSL, the carriers' TLS, builds with CMake, and its bindings with Clang.
RUN apt-get update && apt-get install -y --no-install-recommends cmake clang pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p klue-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/klue-server /usr/local/bin/klue-server
ENV KLUE_DATA=/data TZ=Africa/Cairo
VOLUME /data
EXPOSE 8443
HEALTHCHECK --interval=1m --timeout=5s --start-period=20s CMD ["klue-server", "health"]
ENTRYPOINT ["klue-server"]
