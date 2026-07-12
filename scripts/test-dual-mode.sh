#!/bin/bash
# Dual-mode testing script for lstr - ensures classic and TUI modes are consistent

set -e

echo "🔄 Testing dual-mode consistency..."

# Ensure we're in the right directory
if [ ! -d "examples/sample-directory" ]; then
    echo "❌ Error: examples/sample-directory not found. Run from project root."
    exit 1
fi

# Build the project
echo "🔨 Building project..."
cargo build

echo "📋 Testing classic mode outputs..."

# Generate classic mode outputs
cargo run examples/sample-directory -L 2 > /tmp/lstr-classic-depth2.txt
cargo run examples/sample-directory -G -p -L 2 > /tmp/lstr-classic-flags.txt
cargo run examples/sample-directory --dirs-first -L 2 > /tmp/lstr-classic-dirs.txt

echo "✅ Classic mode tests complete"

echo ""
echo "🖥️  Manual TUI Testing Required:"
echo "   Run these commands and verify the tree structure matches classic mode:"
echo ""
echo "   1. Basic TUI (should match /tmp/lstr-classic-depth2.txt):"
echo "      cargo run examples/sample-directory i -L 2"
echo ""
echo "   2. TUI with flags (should match /tmp/lstr-classic-flags.txt structure):"
echo "      cargo run examples/sample-directory i -G -p"
echo ""
echo "   3. TUI with dirs-first (should match /tmp/lstr-classic-dirs.txt structure):"
echo "      cargo run examples/sample-directory i --dirs-first -L 2"
echo ""
echo "🔍 Check for:"
echo "   • Same tree connector patterns (├── and └──)"
echo "   • Same file ordering within directories"
echo "   • Same directory structure and nesting"
echo "   • Proper alignment of permissions and git status"
echo ""
echo "📁 Classic outputs saved to /tmp/lstr-classic-*.txt for reference"

# If baseline files exist, compare
if [ -f "docs/baseline-outputs/depth-2.txt" ]; then
    echo ""
    echo "📊 Baseline Comparison:"
    if diff -q docs/baseline-outputs/depth-2.txt /tmp/lstr-classic-depth2.txt > /dev/null; then
        echo "   ✅ Classic depth-2 matches baseline"
    else
        echo "   ❌ Classic depth-2 differs from baseline"
        echo "      Run: diff docs/baseline-outputs/depth-2.txt /tmp/lstr-classic-depth2.txt"
    fi
fi