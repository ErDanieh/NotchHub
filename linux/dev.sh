#!/bin/zsh
# Runs a command inside the Linux dev container (see Dockerfile). Example: ./dev.sh cargo test -p notchhub-core
cd "$(dirname "$0")/.."
docker run --rm -t -v "$PWD":/work -v nh-registry:/root/.cargo/registry -v nh-target:/work/linux/target \
  -w /work/linux ${=NH_DOCKER_ARGS} notchhub-linux "$@"
