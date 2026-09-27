#!/usr/bin/env bash
set -e

REPO="${REPO:-capydev42/ctrlr}"
INSTALL_DIR="${INSTALL_DIR:-}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

usage() {
    cat <<EOF
Usage: install.sh [OPTIONS]

Install ctrlr from GitHub releases.

OPTIONS:
    -h, --help              Show this help message
    -d, --dir DIR           Install directory (default: ask user)
    -v, --version VERSION   Specific version to install (default: latest)

EXAMPLES:
    # Install latest version to ~/.local/bin
    curl -fsSL https://github.com/${REPO}/releases/latest/download/install.sh | bash

    # Install to /usr/local/bin (requires sudo)
    curl -fsSL https://github.com/${REPO}/releases/latest/download/install.sh | sudo bash

    # Install specific version
    curl -fsSL https://github.com/${REPO}/releases/download/v0.1.0/install.sh | bash

ENVIRONMENT:
    REPO        GitHub repository (default: capydev42/ctrlr)
    INSTALL_DIR Install directory (default: ask user)
    BASE_URL    Where to fetch the assets from, overriding REPO and --version
EOF
}

# "<sha256>  <asset>" lines, the same file render-formula.sh reads. A missing
# entry is an error, not a skipped check.
expected_sha() {
    local asset="$1" file="$2" sha
    sha="$(awk -v a="$asset" '$2 == a { print $1 }' "$file")"
    if [[ -z "${sha}" ]]; then
        echo -e "${RED}Error: checksums.txt has no entry for ${asset}${NC}" >&2
        return 1
    fi
    printf '%s' "${sha}"
}

# sha256sum on Linux, shasum on macOS, openssl as a third try.
actual_sha() {
    local file="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "${file}" | awk '{ print $1 }'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "${file}" | awk '{ print $1 }'
    elif command -v openssl >/dev/null 2>&1; then
        openssl dgst -sha256 "${file}" | awk '{ print $NF }'
    else
        echo -e "${RED}Error: no sha256 tool found (looked for sha256sum, shasum, openssl)${NC}" >&2
        return 1
    fi
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            exit 0
            ;;
        -d|--dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        -v|--version)
            VERSION="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Detect OS
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux*)
        if [[ "$ARCH" == "aarch64" || "$ARCH" == "arm64" ]]; then
            ASSET_NAME="ctrlr-aarch64-unknown-linux-gnu.tar.gz"
        else
            ASSET_NAME="ctrlr-x86_64-unknown-linux-gnu.tar.gz"
        fi
        ;;
    Darwin*)
        if [[ "$ARCH" == "arm64" ]]; then
            ASSET_NAME="ctrlr-aarch64-apple-darwin.tar.gz"
        else
            ASSET_NAME="ctrlr-x86_64-apple-darwin.tar.gz"
        fi
        ;;
    MINGW*|MSYS*|CYGWIN*)
        echo -e "${RED}Error: this script installs the unix binaries.${NC}"
        echo "On Windows, run in PowerShell:"
        echo "  irm https://github.com/${REPO}/releases/latest/download/install.ps1 | iex"
        exit 1
        ;;
    *)
        echo -e "${RED}Error: Unsupported OS: $OS${NC}"
        exit 1
        ;;
esac

# Determine download URL. BASE_URL wins, which is how the CI check points this
# at a local release over file://.
DOWNLOAD_BASE="${BASE_URL:-}"
if [[ -z "${DOWNLOAD_BASE}" ]]; then
    if [[ -n "${VERSION}" ]]; then
        DOWNLOAD_BASE="https://github.com/${REPO}/releases/download/${VERSION}"
    else
        DOWNLOAD_BASE="https://github.com/${REPO}/releases/latest/download"
    fi
fi
DOWNLOAD_URL="${DOWNLOAD_BASE}/${ASSET_NAME}"

# Determine install directory if not set
if [[ -z "${INSTALL_DIR}" ]]; then
    # Check if stdin is a terminal
    if [[ -t 0 ]]; then
        echo -e "${YELLOW}Where would you like to install ctrlr?${NC}"
        echo "  1) ~/.local/bin (user, no sudo needed)"
        echo "  2) /usr/local/bin (system-wide, requires sudo)"
        echo "  3) Custom path"
        read -p "Enter choice [1]: " choice
        
        case "${choice}" in
            2)
                INSTALL_DIR="/usr/local/bin"
                ;;
            3)
                read -p "Enter custom path: " INSTALL_DIR
                ;;
            *)
                INSTALL_DIR="${HOME}/.local/bin"
                ;;
        esac
    else
        # Nothing to ask with, and no default: INSTALL_DIR is empty or we would
        # not be in this branch.
        echo -e "${RED}Error: Interactive input not available.${NC}"
        echo ""
        echo "When piping to bash, use INSTALL_DIR environment variable:"
        echo "  INSTALL_DIR=~/.local/bin curl -fsSL ... | bash"
        echo "  INSTALL_DIR=/usr/local/bin curl -fsSL ... | sudo bash"
        echo ""
        echo "Or download the script first and run it directly:"
        echo "  curl -fsSL ... -o install.sh && chmod +x install.sh && ./install.sh"
        exit 1
    fi
fi

# Resolve ~ in path
INSTALL_DIR="${INSTALL_DIR/#\~/$HOME}"

echo -e "${YELLOW}Installing ctrlr to ${INSTALL_DIR}...${NC}"

# Create directory if it doesn't exist
mkdir -p "${INSTALL_DIR}"

# Download and extract
TMP_DIR=$(mktemp -d)
# One cleanup for every exit below, of which the checksum check adds three.
trap 'cd /; rm -rf "${TMP_DIR}"' EXIT
cd "${TMP_DIR}"

echo -e "Downloading ${ASSET_NAME}..."
if ! curl -fsSL "${DOWNLOAD_URL}" -o "${ASSET_NAME}"; then
    echo -e "${RED}Error: Failed to download from ${DOWNLOAD_URL}${NC}"
    echo "This might mean the release is not yet available."
    exit 1
fi

# Check if file is an HTML error page
if grep -q "<!DOCTYPE" "${ASSET_NAME}" 2>/dev/null; then
    echo -e "${RED}Error: Received HTML instead of archive (release might not exist)${NC}"
    exit 1
fi

echo -e "Verifying checksum..."
if ! curl -fsSL "${DOWNLOAD_BASE}/checksums.txt" -o checksums.txt; then
    echo -e "${RED}Error: Failed to download checksums.txt from ${DOWNLOAD_BASE}${NC}"
    exit 1
fi

EXPECTED_SHA="$(expected_sha "${ASSET_NAME}" checksums.txt)" || exit 1
ACTUAL_SHA="$(actual_sha "${ASSET_NAME}")" || exit 1

if [[ "${EXPECTED_SHA}" != "${ACTUAL_SHA}" ]]; then
    echo -e "${RED}Error: checksum mismatch for ${ASSET_NAME}${NC}"
    echo "  expected ${EXPECTED_SHA}"
    echo "  got      ${ACTUAL_SHA}"
    exit 1
fi

tar -xzf "${ASSET_NAME}"
rm -f "${INSTALL_DIR}/ctrlr" 2>/dev/null || true
mv ctrlr "${INSTALL_DIR}/"
chmod +x "${INSTALL_DIR}/ctrlr"

echo -e "${GREEN}Installed ctrlr to ${INSTALL_DIR}/ctrlr${NC}"

# Check if in PATH
if [[ ":$PATH:" == *":${INSTALL_DIR}:"* ]]; then
    echo -e "${GREEN}ctrlr is in your PATH. Run 'ctrlr' to start.${NC}"
else
    echo -e "${YELLOW}Note: ${INSTALL_DIR} is not in your PATH.${NC}"
    echo "Add this to your shell config:"
    echo "  export PATH=\"\${HOME}/.local/bin:\$PATH\""
fi
