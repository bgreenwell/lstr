#!/bin/bash
# Basic validation script for lstr functionality

set -e

echo "🔍 Validating basic lstr functionality..."

# Ensure we're in the right directory
if [ ! -d "examples/sample-directory" ]; then
    echo "❌ Error: examples/sample-directory not found. Run from project root."
    exit 1
fi

# Build the project
echo "🔨 Building project..."
cargo build

echo "📋 Running basic validation tests..."

# Test 1: Basic tree structure
echo "  • Testing basic tree structure..."
cargo run examples/sample-directory > /tmp/lstr-basic.txt
if [ $? -ne 0 ]; then
    echo "❌ Basic tree test failed"
    exit 1
fi

# Test 2: Depth limiting
echo "  • Testing depth limiting (-L 2)..."
cargo run examples/sample-directory -L 2 > /tmp/lstr-depth2.txt
if [ $? -ne 0 ]; then
    echo "❌ Depth limiting test failed"
    exit 1
fi

# Test 3: With flags
echo "  • Testing with flags (-G -p -L 2)..."
cargo run examples/sample-directory -G -p -L 2 > /tmp/lstr-flags.txt
if [ $? -ne 0 ]; then
    echo "❌ Flags test failed"
    exit 1
fi

# Test 4: Dirs-first sorting
echo "  • Testing dirs-first sorting..."
cargo run examples/sample-directory --dirs-first -L 2 > /tmp/lstr-dirs-first.txt
if [ $? -ne 0 ]; then
    echo "❌ Dirs-first test failed"
    exit 1
fi

# Test 5: Natural sorting
echo "  • Testing natural sorting..."
cargo run examples/sample-directory --natural-sort -L 2 > /tmp/lstr-natural.txt
if [ $? -ne 0 ]; then
    echo "❌ Natural sorting test failed"
    exit 1
fi

echo "✅ All basic tests passed!"

# Compare with baselines if they exist
if [ -f "docs/baseline-outputs/depth-2.txt" ]; then
    echo "📊 Comparing with baseline outputs..."
    
    if diff -q docs/baseline-outputs/depth-2.txt /tmp/lstr-depth2.txt > /dev/null; then
        echo "  ✅ Depth-2 output matches baseline"
    else
        echo "  ⚠️  Depth-2 output differs from baseline:"
        echo "     Run: diff docs/baseline-outputs/depth-2.txt /tmp/lstr-depth2.txt"
    fi
    
    if [ -f "docs/baseline-outputs/depth-2-with-flags.txt" ]; then
        if diff -q docs/baseline-outputs/depth-2-with-flags.txt /tmp/lstr-flags.txt > /dev/null; then
            echo "  ✅ Flags output matches baseline"
        else
            echo "  ⚠️  Flags output differs from baseline:"
            echo "     Run: diff docs/baseline-outputs/depth-2-with-flags.txt /tmp/lstr-flags.txt"
        fi
    fi
    
    if [ -f "docs/baseline-outputs/depth-2-dirs-first.txt" ]; then
        if diff -q docs/baseline-outputs/depth-2-dirs-first.txt /tmp/lstr-dirs-first.txt > /dev/null; then
            echo "  ✅ Dirs-first output matches baseline"
        else
            echo "  ⚠️  Dirs-first output differs from baseline:"
            echo "     Run: diff docs/baseline-outputs/depth-2-dirs-first.txt /tmp/lstr-dirs-first.txt"
        fi
    fi
    
    if [ -f "docs/baseline-outputs/depth-2-natural-sort.txt" ]; then
        if diff -q docs/baseline-outputs/depth-2-natural-sort.txt /tmp/lstr-natural.txt > /dev/null; then
            echo "  ✅ Natural sorting output matches baseline"
        else
            echo "  ⚠️  Natural sorting output differs from baseline:"
            echo "     Run: diff docs/baseline-outputs/depth-2-natural-sort.txt /tmp/lstr-natural.txt"
        fi
    fi
fi

echo "🎉 Validation complete! Check TUI mode manually with:"
echo "   cargo run examples/sample-directory i -L 2"
echo "   cargo run examples/sample-directory i -G -p"
echo "   cargo run examples/sample-directory i --dirs-first -L 2"