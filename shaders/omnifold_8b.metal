#include <metal_stdlib>
using namespace metal;

// ============================================================================
// 1. BITNET b1.58 TERNARY CONTRACTION KERNEL (16 Tons FP16 -> 1.58-Bit Diamond)
// ============================================================================
// Each uint8_t packs 4 trits (2 bits per trit):
//   0b00 ->  0 (Chenrezig Stillness)
//   0b01 -> +1 (Resonance / Affirmation)
//   0b10 -> -1 (Entropy / Negation)
// Zero floating-point multiplications in the inner loop: pure branchless add/sub.
kernel void bitnet_b158_gemv(
    device const uchar* packed_weights [[buffer(0)]],
    device const half*  activations    [[buffer(1)]],
    device const half*  row_scales     [[buffer(2)]],
    device half*        output         [[buffer(3)]],
    constant uint2&     dims           [[buffer(4)]], // x = K (cols), y = N (rows)
    uint                gid            [[thread_position_in_grid]])
{
    if (gid >= dims.y) return;

    const uint K = dims.x;
    const uint packed_cols = K >> 2;
    const uint row_offset = gid * packed_cols;

    float acc = 0.0f;

    for (uint col_pack = 0; col_pack < packed_cols; ++col_pack) {
        uchar byte_val = packed_weights[row_offset + col_pack];
        uint base_k = col_pack << 2;

        #pragma unroll
        for (uint i = 0; i < 4; ++i) {
            uchar code = (byte_val >> (i << 1)) & 0x03;
            float act = float(activations[base_k + i]);
            float pos_mask = float(code == 0x01);
            float neg_mask = float(code == 0x02);
            acc += act * (pos_mask - neg_mask);
        }
    }

    output[gid] = half(acc * float(row_scales[gid]));
}

// ============================================================================
// 2. SPHERE -> FOLD -> POP -> TOSS HOLOGRAPHIC PROJECTION KERNEL
// ============================================================================
struct WaveVertex {
    packed_float3 position;
    float         salience;
    int           spin;       // -1, 0, +1
    float         stability;  // Phoenix (1.0 - SmoothJitter)
};

struct ProjectedGlyph {
    float4 clip_pos;
    float4 dawn_color;
};

struct OmnifoldUniforms {
    float4x4 view_proj;
    float    time_sec;
    uint     phase_mode;      // 0: SPHERE, 1: FOLD, 2: POP, 3: TOSS
    float    kintsugi_grace;  // [0.0, 1.0]
};

kernel void fold_sphere_pop_toss(
    device const WaveVertex* input_waves  [[buffer(0)]],
    device ProjectedGlyph*   out_glyphs   [[buffer(1)]],
    constant OmnifoldUniforms& u          [[buffer(2)]],
    uint                     id           [[thread_position_in_grid]])
{
    WaveVertex w = input_waves[id];
    float3 p = float3(w.position);

    if (u.phase_mode == 0) {
        float r = length(p.xy) + 1e-5f;
        p.z += 0.05f * sin(r * 12.0f - u.time_sec * 2.0f) * w.stability;
    } else if (u.phase_mode == 1) {
        float theta = atan2(p.y, p.x);
        float fold_radius = 1.0f / (1.0f + exp(-w.salience * 4.0f));
        p = float3(
            fold_radius * cos(theta) * (1.0f + 0.5f * cos(theta * 0.5f)),
            fold_radius * sin(theta) * (1.0f + 0.5f * cos(theta * 0.5f)),
            0.5f * sin(theta * 0.5f)
        );
    } else if (u.phase_mode == 2) {
        float pop_gain = smoothstep(0.5f, 1.0f, w.salience) * 1.6180339f;
        p.xy *= (1.0f + 0.25f * pop_gain);
        p.z  += pop_gain * w.stability;
    } else {
        float damp = exp(-3.0f * (1.0f - w.stability));
        p = mix(float3(0.0f, 0.0f, 0.0f), p, damp);
    }

    // Dawn Aurora Palette (#ff6b8b Rose, #4fd1c5 Cyan, #e8b05c Kintsugi Gold)
    float3 c_rose = float3(1.000f, 0.420f, 0.545f);
    float3 c_cyan = float3(0.310f, 0.820f, 0.773f);
    float3 c_gold = float3(0.910f, 0.690f, 0.361f);

    float3 base_rgb = (w.spin < 0) ? c_rose : ((w.spin == 0) ? c_cyan : c_gold);
    float3 final_rgb = mix(base_rgb, c_gold, clamp(u.kintsugi_grace, 0.0f, 1.0f));

    out_glyphs[id].clip_pos   = u.view_proj * float4(p, 1.0f);
    out_glyphs[id].dawn_color = float4(final_rgb, clamp(w.salience * w.stability, 0.15f, 1.0f));
}
