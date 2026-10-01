FROM node:22-trixie@sha256:ae3b84a81a94a99f50110663c936ccda31a3bbf1e91bb6b23c638669f777a6f2 AS node
FROM ubuntu:24.04@sha256:008173c23f95b170204355c12626cb5a965d779a7e1283b09e9cffbb1bf33ca3
COPY --from=node /usr/local/ /usr/local/
RUN apt-get update && apt-get install -y --no-install-recommends libsqlite3-0 ca-certificates
COPY sdk.tgz /opt/sdk.tgz
RUN npm install --prefix /opt/consumer --ignore-scripts --no-audit --no-fund /opt/sdk.tgz
