//! Independent Zig 0.16 cross-verifier for the Dream Phase's integer contracts.
//!
//! It reimplements the two primitives the Rust crate relies on for identity:
//! FNV-1a 64 payload hashing and the ppm renormaliser. If the two languages
//! disagree on these vectors, the summaries and weights are not portable, so
//! this is the cheap guard that keeps a future Rust -> Zig port honest.
//!
//!     zig run zig/verify_consolidation.zig

const std = @import("std");

fn fnv1a(bytes: []const u8) u64 {
    var hash: u64 = 0xcbf29ce484222325;
    for (bytes) |byte| {
        hash ^= @as(u64, byte);
        hash *%= 0x100000001b3;
    }
    return hash;
}

/// Scale weights so they sum to exactly 1_000_000, distributing the remainder
/// to the earliest buckets — the same rule as `graph::renormalize`.
fn renormalize(weights: []u64) void {
    var total: u128 = 0;
    for (weights) |w| total += w;
    if (total == 0) {
        for (weights) |*w| w.* = 0;
        return;
    }
    var assigned: u64 = 0;
    for (weights) |*w| {
        const scaled: u64 = @intCast(@as(u128, w.*) * 1_000_000 / total);
        w.* = scaled;
        assigned += scaled;
    }
    var remainder: u64 = 1_000_000 - assigned;
    for (weights) |*w| {
        if (remainder == 0) break;
        w.* += 1;
        remainder -= 1;
    }
}

/// Vectors produced by the Rust implementation.
const fnv_vectors = [_]struct { input: []const u8, expected: u64 }{
    .{ .input = "", .expected = 0xcbf29ce484222325 },
    .{ .input = "a", .expected = 0xaf63dc4c8601ec8c },
    .{ .input = "episode-0", .expected = 0xd9f3618dc2d75aa7 },
};

pub fn main(init: std.process.Init) !void {
    _ = init;
    var failures: usize = 0;

    for (fnv_vectors) |v| {
        const got = fnv1a(v.input);
        if (got != v.expected) {
            std.debug.print("FAIL fnv1a({s}) = 0x{x} want 0x{x}\n", .{ v.input, got, v.expected });
            failures += 1;
        }
    }

    var weights = [_]u64{ 700, 300 };
    renormalize(&weights);
    var sum: u64 = 0;
    for (weights) |w| sum += w;
    if (sum != 1_000_000) {
        std.debug.print("FAIL renormalize sum = {d}\n", .{sum});
        failures += 1;
    }
    std.debug.print("renormalize([700,300]) = [{d},{d}] sum={d}\n", .{ weights[0], weights[1], sum });

    var zeros = [_]u64{ 0, 0 };
    renormalize(&zeros);
    if (zeros[0] != 0 or zeros[1] != 0) {
        std.debug.print("FAIL renormalize zeros\n", .{});
        failures += 1;
    }

    if (failures != 0) {
        std.debug.print("{d} failure(s)\n", .{failures});
        std.process.exit(1);
    }
    std.debug.print("zig cross-verifier: ok\n", .{});
}
