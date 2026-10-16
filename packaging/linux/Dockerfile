# A Linux build environment for Whiplash, for building the Linux packages
# from any computer with Docker:
#
#   docker build -t whiplash-linux packaging/linux
#   docker run --rm -v "$PWD":/src:ro -v "$PWD/dist":/out whiplash-linux
#
# Extra Cargo options go in CARGO_ARGS, e.g. `-e CARGO_ARGS=--no-default-features`
# to leave out MilkDrop.
#
# The packages land in ./dist. Ubuntu 22.04 is used so they also run on
# older distributions.
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates curl build-essential cmake pkg-config \
        libasound2-dev libgl-dev libegl-dev libclang-dev librsvg2-bin file rsync \
    && rm -rf /var/lib/apt/lists/*

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --component clippy
ENV PATH=/root/.cargo/bin:$PATH
RUN cargo install cargo-deb --locked
ENV CARGO_ARGS=""

# Copy the source (mounted read-only at /src) so the build doesn't touch
# it, check it, build the packages, and copy them out.
CMD rsync -a --exclude target --exclude .git /src/ /work/ \
    && cd /work \
    && cargo clippy --all-targets $CARGO_ARGS -- -D warnings \
    && cargo test $CARGO_ARGS \
    && packaging/linux/build-packages.sh $CARGO_ARGS \
    && cp target/release/bundle/*.deb target/release/bundle/*.AppImage /out/
