FROM debian:13-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        sudo curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/getappz/agentflare/main/install.sh | sh

ENV PATH="/root/.local/bin:${PATH}"

ENTRYPOINT ["agentflare"]
