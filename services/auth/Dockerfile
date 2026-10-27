# 构建阶段：单仓库（monorepo）布局
# 构建上下文 = 仓库根目录（需要根 Cargo.toml/Cargo.lock 与 libs/ 共享库）：
#   docker build -f services/auth/Dockerfile -t club-oa/auth:local .
FROM rust:1.98-slim-bookworm AS builder
WORKDIR /src
# 工作区清单 + 全部成员清单（cargo 解析 workspace 要求成员目录存在）
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services
RUN cargo build --release --locked -p auth-service

# 运行阶段：最小镜像 + 非 root 用户
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 app \
    && mkdir -p /data/keys \
    && chown -R app:app /data
COPY --from=builder /src/target/release/auth-service /usr/local/bin/auth-service
USER app
EXPOSE 8081
ENTRYPOINT ["auth-service"]
