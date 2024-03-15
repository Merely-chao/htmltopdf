####################################################################################################
## Builder
####################################################################################################
FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev
RUN update-ca-certificates


# Create appuser
ENV USER=htmltopdf
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"


WORKDIR /htmltopdf

COPY ./ .

RUN cargo build --target x86_64-unknown-linux-musl --release

####################################################################################################
## Final image
####################################################################################################
FROM alpine:latest


# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

# Installs latest Chromium package.
RUN apk upgrade --no-cache --available \
    && apk add --no-cache \
      chromium-swiftshader 
    #   \
    #   ttf-freefont \
    #   font-noto-emoji \
    # && apk add --no-cache \
    #   --repository=https://dl-cdn.alpinelinux.org/alpine/edge/testing \
    #   font-wqy-zenhei

COPY local.conf /etc/fonts/local.conf
COPY fonts/* /usr/share/fonts/
# Add Chrome as a user
# RUN mkdir -p /usr/src/app \
#     && chown -R htmltopdf:htmltopdf /usr/src/app
# Run Chrome as non-privileged
USER htmltopdf:htmltopdf
WORKDIR /usr/src/app

ENV CHROME_BIN=/usr/bin/chromium-browser \
    CHROME_PATH=/usr/lib/chromium/ \
    CHROME=/usr/bin/chromium-browser

# Autorun chrome headless
ENV CHROMIUM_FLAGS="--disable-software-rasterizer --disable-dev-shm-usage"



WORKDIR /htmltopdf

# Copy our build
COPY --from=builder /htmltopdf/target/x86_64-unknown-linux-musl/release/htmltopdf ./

# Use an unprivileged user.

EXPOSE 3000

CMD ["/htmltopdf/htmltopdf"]
