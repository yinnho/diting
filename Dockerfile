# diting — browser engine for AI agents, in a container.
# Fetches the static release binary instead of compiling V8 in-image:
# the engine ships as musl (arm64) / gnu (amd64) builds, so a build is a
# download. arm64 uses the musl static screenshot build (the image ships fontconfig + CJK, so the render stack works);
# amd64 uses the gnu build on glibc.
FROM debian:bookworm-slim
ARG DITING_VERSION=0.8.3
ARG TARGETARCH
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates wget xz-utils fontconfig fonts-noto-cjk \
 && rm -rf /var/lib/apt/lists/* \
 && case "${TARGETARCH}" in \
      arm64) ASSET="diting-${DITING_VERSION}-aarch64-unknown-linux-musl-screenshot" ;; \
      amd64) ASSET="diting-v${DITING_VERSION}-x86_64-unknown-linux-gnu.tar.gz" ;; \
      *) echo "unsupported arch ${TARGETARCH}" && exit 1 ;; \
    esac \
 && if [ "${TARGETARCH}" = "arm64" ]; then \
        wget -qO /usr/local/bin/diting \
          "https://github.com/yinnho/diting/releases/download/v${DITING_VERSION}/${ASSET}"; \
    else \
        wget -q "https://github.com/yinnho/diting/releases/download/v${DITING_VERSION}/${ASSET}" \
        && tar xzf "${ASSET}" \
        && mv "diting-v${DITING_VERSION}-x86_64-unknown-linux-gnu/diting" /usr/local/bin/diting \
        && rm -rf "${ASSET}" "diting-v${DITING_VERSION}-x86_64-unknown-linux-gnu"; \
    fi \
 && chmod +x /usr/local/bin/diting

# CJK fonts ride along for the render path (fetch/search never need them).
# Bind 0.0.0.0 inside the container; pair with DITING_TOKEN in production.
ENV DITING_BIND=0.0.0.0:8089
EXPOSE 8089
ENTRYPOINT ["/usr/local/bin/diting"]
