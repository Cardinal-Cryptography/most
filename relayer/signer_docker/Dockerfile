FROM ubuntu:jammy-20231211.1

COPY signer /signer/signer
COPY entrypoint.sh /signer/entrypoint.sh

WORKDIR /signer
ENTRYPOINT ["/signer/entrypoint.sh"]
