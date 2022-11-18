CONTAINER_NAME     := delegator
BIN_NAME           := delegator
ALPINE_TAG     	   := latest
DOCKER_BUILDKIT    := 1

.PHONY: all docker_build docker_build_debug docker_build_release 

all:

docker_build: docker_build_release

docker_build_debug: Dockerfile
	DOCKER_BUILDKIT=$(DOCKER_BUILDKIT) \
	docker -D build \
		--build-arg ALPINE_TAG='$(ALPINE_TAG)' \
		--build-arg BIN_NAME='$(BIN_NAME)' \
		--build-arg RUST_STAGE='debug' \
		--tag $(CONTAINER_NAME) \
		.

docker_build_release: Dockerfile
	DOCKER_BUILDKIT=$(DOCKER_BUILDKIT) \
	docker -D build \
		--build-arg ALPINE_TAG='$(ALPINE_TAG)' \
		--build-arg BIN_NAME='$(BIN_NAME)' \
		--build-arg RUST_STAGE='release' \
		--tag $(CONTAINER_NAME) \
		.
