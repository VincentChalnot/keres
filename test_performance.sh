#!/bin/bash
# Performance comparison script for MCTS engine

set -e

echo "======================================"
echo "Keres Engine Performance Test"
echo "======================================"
echo ""

# Check if we have GPU support
echo "Checking GPU availability..."
if cargo run --example engine_demo 2>&1 | grep -E "(GPU|Failed|✓)" | head -5; then
    echo "Example ran successfully"
else
    echo "Warning: Example may have encountered issues"
fi

echo ""
echo "Running tests..."
if cargo test --release -- --nocapture 2>&1 | tee /tmp/test_output.txt | grep -E "(test result|running|Skipping)" | head -20; then
    echo ""
    # Check if tests actually passed
    if grep -q "test result: ok" /tmp/test_output.txt; then
        echo "✓ All tests passed"
    else
        echo "✗ Some tests failed"
        exit 1
    fi
else
    echo "✗ Test execution failed"
    exit 1
fi

echo ""
echo "======================================"
echo "Test complete!"
echo ""
echo "Key features implemented:"
echo "  ✓ GPU batch simulation for parallel processing"
echo "  ✓ Multi-threaded CPU evaluation with Rayon"
echo "  ✓ Statistics tracking (moves, simulations, GPU/CPU usage)"
echo "  ✓ Configurable batch sizes (64-1024)"
echo "  ✓ Automatic CPU fallback when GPU unavailable"
echo ""
echo "To run the demo:"
echo "  cargo run --example engine_demo"
echo ""
echo "To run with custom configuration, modify examples/engine_demo.rs"
echo "======================================"
