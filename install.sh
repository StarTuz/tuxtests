#!/bin/bash
# TuxTests v0.8.1 Installer Script

set -e

echo "🚀 Initializing TuxTests v0.8.1 Build Pipeline..."

# Verify cargo dependency
if ! command -v cargo &> /dev/null; then
    echo "❌ CRITICAL ERROR: 'cargo' could not be found! Please install the Rust toolchain (rustup) to proceed."
    exit 1
fi

echo "📦 Compiling highly-optimized release binary..."
export CARGO_HOME="$PWD/.cargo_home"
cargo build --release

INSTALL_DIR="$HOME/.local/bin"

echo "🚚 Provisioning target directory ($INSTALL_DIR)..."
mkdir -p "$INSTALL_DIR"

echo "⚡ Deploying executable natively..."
cp target/release/tuxtests "$INSTALL_DIR/tuxtests"
chmod +x "$INSTALL_DIR/tuxtests"

echo "✅ TuxTests v0.8.1 Successfully Deployed!"
echo ""
echo "Note: Ensure that '$INSTALL_DIR' evaluates inside your active \$PATH!"
echo ""
echo "To initialize the Secure Gemini Pipeline, run:"
echo "  tuxtests --set-gemini-key \"YOUR_API_KEY_HERE\""
echo ""
echo "Or, configure for fully offline Local Privacy execution via Ollama:"
echo "  tuxtests --set-llm-provider ollama --set-ollama-url http://127.0.0.1:11434 --set-ollama-model mistral"
echo ""
echo "To launch your first native analysis, type:"
echo "  tuxtests --analyze"
