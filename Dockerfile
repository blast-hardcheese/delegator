ARG BASE_IMAGE
ARG BASE_IMAGE_TAG
FROM --platform=${BUILDPLATFORM} ${BASE_IMAGE}:${BASE_IMAGE_TAG}

ARG RUST_STAGE

WORKDIR /app

# Copy the binary and configuration
COPY config/ /app/config/
COPY target/${RUST_STAGE}/delegator Cargo.* /app/

EXPOSE 80/tcp
ENV RUST_BACKTRACE=1

# When `docker run` is executed, launch the binary!
CMD [ "/app/delegator", "config/application.conf" ]
