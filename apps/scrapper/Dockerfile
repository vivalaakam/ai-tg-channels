# Builds the scrapper binary from the workspace.
# libtdjson.so (TDLib) is downloaded at build time from the
# @prebuilt-tdlib/linux-arm64-glibc npm tarball (same source the TS project
# uses on macOS via darwin-arm64) and shipped in the runtime image.
# Runtime needs: DATABASE_URL, TG_API_ID, TG_API_HASH, TG_PHONE (see .env.example).
# TDLib session state lives in /data (mount a volume there).
FROM rust:1-bookworm AS build

WORKDIR /src

ARG TDJSON_VERSION=0.1008070.0
ENV TDJSON_VERSION=${TDJSON_VERSION}

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps

# Fetch libtdjson.so for linux/arm64 glibc into /out so the runtime stage can pick it up
RUN mkdir -p /out \
    && curl -fsSL \
      "https://registry.npmjs.org/@prebuilt-tdlib/linux-arm64-glibc/-/linux-arm64-glibc-${TDJSON_VERSION}.tgz" \
      -o /tmp/tdlib.tgz \
    && tar -xzf /tmp/tdlib.tgz -C /tmp package/libtdjson.so \
    && mv /tmp/package/libtdjson.so /out/libtdjson.so

# -j2: pikes RAM, VM build hosts are memory-limited (kill 137 on 1.9GB)
RUN cargo build --release --locked -j 2 -p scrapper

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --user-group --shell /usr/sbin/nologin tgscrap

COPY --from=build /out/libtdjson.so /usr/local/lib/libtdjson.so
RUN ldconfig

COPY --from=build /src/target/release/scrapper /usr/local/bin/scrapper

USER tgscrap

WORKDIR /data

VOLUME ["/data"]

# Required at runtime: DATABASE_URL, TG_API_ID, TG_API_HASH, TG_PHONE
# TDJSON_PATH points inside the image; auth flow reads stdin only when a code
# is requested, normally not needed if /data already holds an authorized session.
ENV TDJSON_PATH=/usr/local/lib/libtdjson.so \
    TDLIB_DATA_DIR=/data/.tdlib \
    TDLIB_FILES_DIR=/data/.tdlib-files

ENTRYPOINT ["/usr/local/bin/scrapper"]
