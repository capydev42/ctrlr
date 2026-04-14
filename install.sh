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
EOF
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
        ASSET_NAME="ctrlr-x86_64-unknown-linux-gnu.tar.gz"
        ;;
    Darwin*)
        if [[ "$ARCH" == "arm64" ]]; then
            ASSET_NAME="ctrlr-aarch64-apple-darwin.tar.gz"
        else
            ASSET_NAME="ctrlr-x86_64-apple-darwin.tar.gz"
        fi
        ;;
    *)
        echo -e "${RED}Error: Unsupported OS: $OS${NC}"
        exit 1
        ;;
esac

# Determine download URL
if [[ -n "${VERSION}" ]]; then
    DOWNLOAD_BASE="https://github.com/${REPO}/releases/download/${VERSION}"
else
    DOWNLOAD_BASE="https://github.com/${REPO}/releases/latest/download"
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
        # Non-interactive: use default or fail with helpful message
        if [[ -n "${INSTALL_DIR}" ]]; then
            echo -e "${YELLOW}Using INSTALL_DIR=${INSTALL_DIR}${NC}"
        else
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
fi

# Resolve ~ in path
INSTALL_DIR="${INSTALL_DIR/#\~/$HOME}"

echo -e "${YELLOW}Installing ctrlr to ${INSTALL_DIR}...${NC}"

# Create directory if it doesn't exist
mkdir -p "${INSTALL_DIR}"

# Download and extract
TMP_DIR=$(mktemp -d)
cd "${TMP_DIR}"

echo -e "Downloading ${ASSET_NAME}..."
if ! curl -fsSL "${DOWNLOAD_URL}" -o "${ASSET_NAME}"; then
    echo -e "${RED}Error: Failed to download from ${DOWNLOAD_URL}${NC}"
    echo "This might mean the release is not yet available."
    rm -rf "${TMP_DIR}"
    exit 1
fi

# Check if file is an HTML error page
if grep -q "<!DOCTYPE" "${ASSET_NAME}" 2>/dev/null; then
    echo -e "${RED}Error: Received HTML instead of archive (release might not exist)${NC}"
    rm -rf "${TMP_DIR}"
    exit 1
fi

tar -xzf "${ASSET_NAME}"
rm -f "${INSTALL_DIR}/ctrlr" 2>/dev/null || true
mv ctrlr "${INSTALL_DIR}/"
chmod +x "${INSTALL_DIR}/ctrlr"

# Cleanup
cd /
rm -rf "${TMP_DIR}"

echo -e "${GREEN}Installed ctrlr to ${INSTALL_DIR}/ctrlr${NC}"

# Check if in PATH
if [[ ":$PATH:" == *":${INSTALL_DIR}:"* ]]; then
    echo -e "${GREEN}ctrlr is in your PATH. Run 'ctrlr' to start.${NC}"
else
    echo -e "${YELLOW}Note: ${INSTALL_DIR} is not in your PATH.${NC}"
    echo "Add this to your shell config:"
    echo "  export PATH=\"\${HOME}/.local/bin:\$PATH\""
fi
