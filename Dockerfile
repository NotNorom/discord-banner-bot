FROM docker.io/rust:1.78.0-buster as builder

WORKDIR /app
COPY . .

# RUN rustup target add x86_64-unknown-linux-musl
# RUN cargo build --release --target=x86_64-unknown-linux-musl
RUN cargo build --release

# runner image
FROM docker.io/debian:buster-slim

ARG APP_DIR=/app
ARG CONFIG_DIR=/conf
ARG DATABASE_DIR=/database

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC \
    APP_USER=dbb

RUN groupadd ${APP_USER} \
    && useradd -g ${APP_USER} ${APP_USER}

# create APP directory, copy binaries and set permissions/ownership
RUN mkdir ${APP_DIR}
COPY --from=builder /app/target/release/bot ${APP_DIR}/bot
COPY --from=builder /app/target/release/util ${APP_DIR}/util
RUN chown -R ${APP_USER}:${APP_USER} $APP_DIR && chmod +x ${APP_DIR}/*

# create CONF directory, copy config files and set permissions/ownership
RUN mkdir /conf
COPY ./settings.template.toml ${CONFIG_DIR}/settings.toml
RUN chown -R ${APP_USER}:${APP_USER} /conf
VOLUME [ "/conf" ]

# create database folder (junst in case we'll need in in the future)
RUN mkdir /database && chown -R ${APP_USER}:${APP_USER} ${DATABASE_DIR}
VOLUME [ "/database" ]

USER ${APP_USER}
WORKDIR ${APP_DIR}

CMD ["./bot", "--settings-file", "/conf/settings.toml"]
