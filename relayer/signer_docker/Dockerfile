FROM alpine:latest

RUN apk update && apk add bash
COPY signer /signer/signer
COPY entrypoint.sh /signer/entrypoint.sh

WORKDIR /signer
ENTRYPOINT ["/signer/entrypoint.sh"]
