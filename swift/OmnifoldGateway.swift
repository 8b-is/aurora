import Foundation
import Metal
import simd

/// Sovereign zero-copy Metal gateway for the 8b M1 64GB unified memory architecture.
/// Bridges `aurora-mem8` wave grids and BitNet b1.58 ternary weights into Apple Silicon GPU cores.
public final class OmnifoldGateway {
    public enum GatewayError: Error {
        case metalDeviceUnavailable
        case commandQueueCreationFailed
        case shaderCompilationFailed(String)
        case unifiedBufferAllocationFailed
    }

    public let device: MTLDevice
    public let commandQueue: MTLCommandQueue
    private let gemvPipeline: MTLComputePipelineState
    private let foldPipeline: MTLComputePipelineState

    public init(metalSource: String) throws {
        guard let dev = MTLCreateSystemDefaultDevice() else {
            throw GatewayError.metalDeviceUnavailable
        }
        self.device = dev

        guard let queue = dev.makeCommandQueue() else {
            throw GatewayError.commandQueueCreationFailed
        }
        self.commandQueue = queue

        let options = MTLCompileOptions()
        options.fastMathEnabled = true
        let library = try dev.makeLibrary(source: metalSource, options: options)

        guard let gemvFn = library.makeFunction(name: "bitnet_b158_gemv"),
              let foldFn = library.makeFunction(name: "fold_sphere_pop_toss") else {
            throw GatewayError.shaderCompilationFailed("Missing required Omnifold entrypoints")
        }

        self.gemvPipeline = try dev.makeComputePipelineState(function: gemvFn)
        self.foldPipeline = try dev.makeComputePipelineState(function: foldFn)
    }

    /// Maps an external page-aligned `mmap` pointer (`.aye_consciousness.m8`) into Metal
    /// unified memory with zero CPU<->GPU copies (`storageModeShared`).
    public func bindUnifiedWaveGrid(
        pointer: UnsafeMutableRawPointer,
        byteLength: Int
    ) throws -> MTLBuffer {
        guard let buf = device.makeBuffer(
            bytesNoCopy: pointer,
            length: byteLength,
            options: [.storageModeShared],
            deallocator: nil
        ) else {
            throw GatewayError.unifiedBufferAllocationFailed
        }
        return buf
    }
}
