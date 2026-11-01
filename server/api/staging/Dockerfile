# Runtime image for the staging API: Debian 12 (the binary is built against it) plus the CA certificates the server needs
# to reach OpenRouter and Stripe over TLS. The binary itself is mounted in by compose.yml, not baked in.
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
CMD ["/app/vershion-api"]
