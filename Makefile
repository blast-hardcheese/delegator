CONTAINER_NAME     := delegator
BASE_IMAGE         := ubuntu
BASE_IMAGE_TAG     := latest
DOCKER_BUILDKIT    := 1

.PHONY: all docker_build docker_build_release

all:

docker_build: docker_build_release

docker_build_release: Dockerfile
	DOCKER_BUILDKIT=$(DOCKER_BUILDKIT) \
	docker -D build \
		--build-arg BASE_IMAGE='$(BASE_IMAGE)' \
		--build-arg BASE_IMAGE_TAG='$(BASE_IMAGE_TAG)' \
		--build-arg RUST_STAGE='release' \
		--platform linux/amd64 \
		--tag $(CONTAINER_NAME) \
		.
