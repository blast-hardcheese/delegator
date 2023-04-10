FROM --platform=${BUILDPLATFORM} public.ecr.aws/docker/library/rust:1.67-slim-bookworm as base

ARG CARGO_ARGS
ARG RUST_STAGE

WORKDIR /app

FROM base as deps

# RUN apk add pkgconfig libc-dev openssl-dev gcompat
RUN apt-get update
RUN apt-get install -fy pkg-config libc-dev libssl-dev

# Cache dependencies
COPY Cargo.lock Cargo.toml ./
RUN cargo fetch

FROM deps as build

# Build
COPY src/ src/

RUN cargo build $CARGO_ARGS

RUN mkdir bin
RUN cp -v target/$RUST_STAGE/delegator bin/

FROM --platform=${BUILDPLATFORM} public.ecr.aws/docker/library/rust:1.67-slim-bookworm as service

WORKDIR /app
COPY --from=build /app/bin /app/bin
COPY config ./config

EXPOSE 80/tcp
ENV RUST_BACKTRACE=1

CMD [ "sh", "-c", "exec ./bin/delegator \"config/$ENVIRONMENT.conf\"" ]
