@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;

// vibe coding
const RADIUS = 3;        // Kernel radius (higher = smoother)
const SHARPNESS = 4.0;    // Higher = more detail preserved (8-20)
const SECTORS = 4;        // 4 or 8 sectors
const DITHER_STRENGTH = 0.01; // Adjust for dithering intensity

fn random(seed: vec2<f32>) -> f32 {
    return fract(sin(dot(seed, vec2(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(input_tex));
    let uv = frag_coord.xy / tex_size;
    let texel_size = 1.0 / tex_size;

    var best_mean = vec3(0.0);
    var min_variance = 1e6;

    // Process each sector
    for (var k = 0; k < SECTORS; k++) {
        let angle = f32(k) * 6.283185 / f32(SECTORS); // 2π / SECTORS
        let dir = vec2(cos(angle), sin(angle));

        var mean = vec3(0.0);
        var mean_sq = vec3(0.0);
        var weight = 0.0;

        // Sample sector (square quadrant)
        for (var y = -RADIUS; y <= RADIUS; y++) {
            for (var x = -RADIUS; x <= RADIUS; x++) {
                // Check if pixel is within the current sector
                let pos = vec2(f32(x), f32(y));
                let dot_prod = dot(normalize(pos), dir);
                if (dot_prod < 0.7071) { continue; } // ≈ cos(π/4)

                let sample_uv = uv + pos * texel_size;
                let color = textureSample(input_tex, tex_sampler, sample_uv).rgb;
                let w = 1.0; // Uniform weighting (can use exp(-x²/SHARPNESS))

                mean += color * w;
                mean_sq += color * color * w;
                weight += w;
            }
        }

        mean /= weight;
        let variance = length(mean_sq / weight - mean * mean);

        // Keep sector with lowest variance
        if (variance < min_variance) {
            min_variance = variance;
            best_mean = mean;
        }
    }

    // Apply random dithering
    let dither = (random(frag_coord.xy) - 0.5) * DITHER_STRENGTH;
    let final_color = best_mean + vec3(dither, dither, dither);

    return vec4(final_color, 1.0);
}
