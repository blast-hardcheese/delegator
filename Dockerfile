ARG ALPINE_TAG
FROM --platform=${BUILDPLATFORM} alpine:${ALPINE_TAG}

ARG BIN_NAME
ARG RUST_STAGE

WORKDIR /app

# The gcompat package is requried to allow Alpine to run binaries that need glibc instead of 
# the default 'musl' library
RUN apk add --no-cache gcompat

# Copy the binary and configuration
COPY config/ /app/config/
COPY target/${RUST_STAGE}/${BIN_NAME} Cargo.* /app/

EXPOSE 80/tcp
ENV RUST_BACKTRACE=1

# When `docker run` is executed, launch the binary!
ENTRYPOINT [ "/bin/sh", "-l", "-c" ]
CMD [ "/app/${BIN_NAME}" "config/development.conf" ]
