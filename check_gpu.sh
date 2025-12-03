#!/bin/bash

# GPU Diagnostic Script for Keres Engine
# This script helps diagnose GPU availability issues, especially in containers

echo "================================"
echo "Keres Engine GPU Diagnostic Tool"
echo "================================"
echo ""

# Check if running in Docker
if [ -f /.dockerenv ]; then
    echo "✓ Running inside Docker container"
else
    echo "○ Running on host system (not in container)"
fi
echo ""

# Check Vulkan
echo "--- Checking Vulkan ---"
if command -v vulkaninfo &> /dev/null; then
    echo "✓ vulkaninfo is installed"
    
    # Get basic Vulkan info
    vulkan_output=$(vulkaninfo 2>&1)
    if echo "$vulkan_output" | grep -q "ERROR"; then
        echo "❌ Vulkan initialization failed"
        echo "$vulkan_output" | grep "ERROR" | head -5
    else
        echo "✓ Vulkan is working"
        
        # Extract GPU info
        echo ""
        echo "Available GPUs:"
        echo "$vulkan_output" | grep -A 1 "deviceName" | head -20
    fi
else
    echo "❌ vulkaninfo not found (install vulkan-tools)"
fi
echo ""

# Check GPU devices
echo "--- Checking GPU Devices ---"
if [ -d /dev/dri ]; then
    echo "✓ /dev/dri directory exists"
    ls -la /dev/dri/ 2>/dev/null || echo "  (permission denied or empty)"
else
    echo "❌ /dev/dri not found (GPU device files missing)"
fi
echo ""

# Check for NVIDIA GPU
if command -v nvidia-smi &> /dev/null; then
    echo "--- NVIDIA GPU Info ---"
    nvidia-smi --query-gpu=name,driver_version --format=csv,noheader 2>/dev/null || echo "❌ nvidia-smi failed"
else
    echo "○ nvidia-smi not found (not an NVIDIA GPU or driver not installed)"
fi
echo ""

# Check environment variables
echo "--- Environment Variables ---"
echo "WGPU_BACKEND: ${WGPU_BACKEND:-not set (will try all backends)}"
echo "VK_ICD_FILENAMES: ${VK_ICD_FILENAMES:-not set}"
echo ""

# Test with Keres Engine
echo "--- Testing Keres Engine GPU Detection ---"
if [ -f "./target/debug/keres" ] || [ -f "./target/release/keres" ]; then
    binary=$([ -f "./target/release/keres" ] && echo "./target/release/keres" || echo "./target/debug/keres")
    echo "Running cargo test to check GPU detection..."
    echo "(This will show GPU initialization logs)"
    echo ""
    cargo test --lib test_gpu_context_creation -- --nocapture 2>&1 | grep -E "(Initializing|Found|Selected|adapters|GPU|Failed|ERROR)" | head -20
else
    echo "⚠ Keres binary not found. Run 'cargo build' first."
fi
echo ""

echo "================================"
echo "Diagnostic complete"
echo "================================"
echo ""
echo "Common fixes for container GPU issues:"
echo "1. Run with GPU access: docker run --gpus all ..."
echo "2. Install vulkan-loader in container"
echo "3. Set WGPU_BACKEND=VULKAN environment variable"
echo "4. Check that /dev/dri is accessible in container"
echo ""
