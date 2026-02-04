# Multi-arch container for mockserver
# Built using Podman/Buildah with distroless base for minimal attack surface

FROM gcr.io/distroless/cc-debian12:nonroot

ARG TARGETARCH

# Copy the pre-built binary for the target architecture
# Binaries are staged by CI at binaries/{amd64,arm64}/mockserver
COPY binaries/${TARGETARCH}/mockserver /mockserver

# Copy license files
COPY LICENSE /LICENSE
COPY THIRD_PARTY_LICENSES /THIRD_PARTY_LICENSES

# Default port for the mock server
EXPOSE 3000

# Run as non-root user (provided by distroless:nonroot)
USER nonroot

ENTRYPOINT ["/mockserver"]
CMD ["serve", "--host", "0.0.0.0"]
