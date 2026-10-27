# 构建阶段：单仓库（monorepo）布局，构建上下文 = 仓库根目录
#   docker build -f services/drive/Dockerfile -t club-oa/drive:local .
FROM rust:1.98-slim-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services
RUN cargo build --release --locked -p drive-service

# 运行阶段：最小镜像 + 非 root 用户
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 app \
    && mkdir -p /data/storage /data/uploads \
    && chown -R app:app /data
COPY --from=builder /src/target/release/drive-service /usr/local/bin/drive-service
USER app
EXPOSE 8087
ENTRYPOINT ["drive-service"]
