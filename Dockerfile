# syntax=docker/dockerfile:1
#
# Turu without a window (docs/HEADLESS.md, "Docker"): builds the app from this checkout and runs
# it with `--headless`. Check out the submodules first (`git submodule update --init ruffle`).
#
#   docker build -t turu .
#   docker compose up -d          # compose.yaml, with docker/turu.env
#
# Build arguments: PROFILE (`dist`, as the release packages; `release` builds faster) and
# TURU_VERSION (the version Turu reports and compares with its releases; default the crate's).

ARG DEBIAN=bookworm

FROM rust:1-${DEBIAN} AS build

# .github/linux-dependencies.txt, Java for Ruffle's playerglobal (asc.jar) and libclang for
# jpegxr's bindgen (GitHub's runners have the last two already).
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libasound2-dev libudev-dev libfontconfig-dev libfreetype6-dev libssl-dev \
        openjdk-17-jre-headless libclang-dev cmake pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .

ARG PROFILE=dist
ARG FEATURES=software_video,external_video,lzma,jpegxr,fontconfig-dlopen
ARG TURU_VERSION=
ENV CFG_RELEASE_CHANNEL=stable

# The registry and target folder are build caches, kept between builds; the binary is copied out
# of them in the same step.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    if [ -n "$TURU_VERSION" ]; then export TURU_VERSION; else unset TURU_VERSION; fi \
    && cargo build --locked --package turu --profile "$PROFILE" \
        --no-default-features --features "$FEATURES" \
    && install -D -m 755 "target/$PROFILE/turu" /out/turu \
    && strip --strip-debug /out/turu

FROM debian:${DEBIAN}-slim

# docs/HEADLESS.md's runtime libraries, CA certificates for HTTPS, tini to pass signals on.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libasound2 libudev1 libssl3 libfontconfig1 libfreetype6 ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*

# The program's folder belongs to the user that runs it, so `POST /update` can replace the binary
# (it lasts until the container is recreated: rebuild the image to keep an update).
RUN useradd --system --create-home --home-dir /home/turu turu \
    && install -d -o turu -g turu /opt/turu /data
COPY --from=build --chown=turu:turu /out/turu /opt/turu/turu
COPY --chmod=755 docker/entrypoint.sh /usr/local/bin/turu-entrypoint
COPY README.md LICENSE LICENSE-RUFFLE.md /opt/turu/

USER turu
ENV TURU_DATA_DIR=/data \
    TURU_API=0.0.0.0:9021
VOLUME /data
EXPOSE 9021 8790
WORKDIR /data

ENTRYPOINT ["tini", "--", "turu-entrypoint"]
