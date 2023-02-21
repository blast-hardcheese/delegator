FROM --platform=${BUILDPLATFORM} public.ecr.aws/docker/library/rust:1.67-slim-bookworm as base

ARG CARGO_ARGS
ARG RUST_STAGE

WORKDIR /app

# Cache dependencies
COPY Cargo.lock Cargo.toml ./
RUN cargo fetch

# Build
COPY config/application.conf ./config/application.conf
COPY src/ src/

# RUN apk add pkgconfig libc-dev openssl-dev gcompat
RUN apt-get update
RUN apt-get install -fy pkg-config libc-dev libssl-dev

RUN cargo build $CARGO_ARGS

RUN mkdir bin
RUN cp -v target/$RUST_STAGE/delegator bin/

FROM --platform=${BUILDPLATFORM} public.ecr.aws/docker/library/rust:1.67-slim-bookworm as service

WORKDIR /app
COPY --from=base /app/bin /app/bin
COPY --from=base /app/config /app/config

EXPOSE 80/tcp
ENV RUST_BACKTRACE=1

CMD [ "./bin/delegator", "config/application.conf" ]
