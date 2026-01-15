//! Shared GPU context for WGPU-based compute operations
//!
//! This module provides a centralized GPU adapter and device selection that can be
//! shared across multiple GPU-accelerated engines (move generation, batch simulation, etc.)
//! to ensure they all use the same GPU device.

use std::env;
use std::sync::{Arc, Mutex, OnceLock};

/// Shared GPU context that manages adapter and device selection
#[derive(Clone)]
pub struct GpuContext {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    adapter_info: Arc<wgpu::AdapterInfo>,
}

impl GpuContext {
    /// Create a new GPU context with adapter and device
    pub async fn new() -> Result<Self, String> {
        Self::new_with_label("GPU Context").await
    }

    /// Create a new GPU context with a custom label
    pub async fn new_with_label(label: &str) -> Result<Self, String> {
        // Check for backend preference from environment
        let backends = match env::var("WGPU_BACKEND") {
            Ok(backend) => {
                eprintln!("🔧 WGPU_BACKEND environment variable set to: {}", backend);
                match backend.to_uppercase().as_str() {
                    "VULKAN" => wgpu::Backends::VULKAN,
                    "DX12" => wgpu::Backends::DX12,
                    "METAL" => wgpu::Backends::METAL,
                    "GL" => wgpu::Backends::GL,
                    _ => {
                        eprintln!("⚠ Unknown WGPU_BACKEND '{}', using all backends", backend);
                        wgpu::Backends::all()
                    }
                }
            }
            Err(_) => wgpu::Backends::all(),
        };

        // Initialize wgpu
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        // Enumerate and log available adapters for debugging
        let adapters = instance.enumerate_adapters(backends);
        if adapters.is_empty() {
            eprintln!("❌ No GPU adapters found!");
            eprintln!("   Backends attempted: {:?}", backends);
            eprintln!("   This may indicate:");
            eprintln!("   - No GPU drivers installed");
            eprintln!("   - GPU not exposed to container (missing --device or --gpus flag)");
            eprintln!("   - Vulkan ICD not properly configured");
            eprintln!("   Suggestion: Check 'vulkaninfo' output and Docker GPU configuration");
        } else {
            eprintln!("📊 Found {} GPU adapter(s):", adapters.len());
            for (idx, adapter) in adapters.iter().enumerate() {
                let info = adapter.get_info();
                eprintln!(
                    "   [{}] {} - {:?} ({:?})",
                    idx, info.name, info.device_type, info.backend
                );
            }
        }

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                let error_msg = "Failed to find an appropriate GPU adapter";
                eprintln!("❌ {}", error_msg);
                eprintln!("   Possible causes:");
                eprintln!("   1. No compatible GPU found");
                eprintln!("   2. GPU drivers not installed or outdated");
                eprintln!("   3. Running in container without GPU access");
                eprintln!("   4. Vulkan runtime not properly configured");
                eprintln!();
                eprintln!("   Container troubleshooting:");
                eprintln!("   - Verify GPU is accessible: docker run --gpus all ...");
                eprintln!("   - Check Vulkan: docker run ... vulkaninfo");
                eprintln!("   - Set WGPU_BACKEND env var to force specific backend");
                error_msg.to_string()
            })?;

        let adapter_info = adapter.get_info();
        eprintln!(
            "✓ Selected GPU: {} ({:?})",
            adapter_info.name, adapter_info.backend
        );

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some(label),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to create device: {}", e))?;

        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            adapter_info: Arc::new(adapter_info),
        })
    }

    /// Get a reference to the device
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Get a reference to the queue
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Get adapter information
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.adapter_info
    }

    /// Create a synchronized instance (blocking)
    pub fn new_sync() -> Result<Self, String> {
        pollster::block_on(Self::new())
    }

    /// Create a synchronized instance with custom label (blocking)
    pub fn new_sync_with_label(label: &str) -> Result<Self, String> {
        pollster::block_on(Self::new_with_label(label))
    }
}

// Global shared GPU context
static SHARED_GPU_CONTEXT: OnceLock<Mutex<Option<GpuContext>>> = OnceLock::new();

/// Get or initialize the shared GPU context
///
/// This function ensures that all GPU engines use the same GPU device,
/// which is more efficient and prevents potential resource conflicts.
pub fn get_shared_context() -> Result<GpuContext, String> {
    let mutex = SHARED_GPU_CONTEXT.get_or_init(|| Mutex::new(None));

    let mut guard = mutex
        .lock()
        .map_err(|e| format!("Failed to lock GPU context: {}", e))?;

    if let Some(ref context) = *guard {
        Ok(context.clone())
    } else {
        eprintln!("🔄 Initializing shared GPU context...");
        let context = GpuContext::new_sync_with_label("Shared GPU Context")?;
        *guard = Some(context.clone());
        Ok(context)
    }
}

/// Reset the shared GPU context (mainly useful for testing)
#[cfg(test)]
pub fn reset_shared_context() {
    if let Some(mutex) = SHARED_GPU_CONTEXT.get() {
        if let Ok(mut guard) = mutex.lock() {
            *guard = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_context_creation() {
        let context = GpuContext::new_sync();
        if let Err(e) = &context {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }
        assert!(context.is_ok());
        let ctx = context.unwrap();
        println!("GPU: {}", ctx.adapter_info().name);
    }

    #[test]
    fn test_shared_context() {
        reset_shared_context();

        let ctx1 = get_shared_context();
        if let Err(e) = &ctx1 {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }

        let ctx2 = get_shared_context();
        assert!(ctx2.is_ok());

        // Both contexts should point to the same device
        let ctx1 = ctx1.unwrap();
        let ctx2 = ctx2.unwrap();

        // Compare adapter info to verify they're the same
        assert_eq!(ctx1.adapter_info().name, ctx2.adapter_info().name);
        assert_eq!(ctx1.adapter_info().device, ctx2.adapter_info().device);
    }
}
