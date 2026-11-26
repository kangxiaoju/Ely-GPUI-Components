/// Stable FNV-1a seed for a shape's persistent key, shared by native and export renderers.
pub fn sketch_seed(key: &str) -> u64 {
    key.bytes().fold(0xcbf29ce484222325, |seed, byte| (seed ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}

/// Samples smooth bowed, wavy edges in canvas units, keeping every original vertex.
/// Closed outlines repeat the first point. A second trace uses `seed ^ 0x9e3779b97f4a7c15`.
/// Sampling is bounded per edge and must be cached with scene geometry, not animated.
pub fn sketch_outline(points: &[(f32, f32)], closed: bool, roughness: f32, seed: u64) -> Vec<(f32, f32)> {
    if points.len() < 2 || !roughness.is_finite() || roughness <= 0.0 { return points.to_vec(); }
    let roughness = roughness.min(10.0);
    let edges = points.len() - 1 + usize::from(closed);
    let mut result = Vec::with_capacity(edges * 17 + 1);
    result.push(points[0]);
    for edge in 0..edges {
        let (a,b) = (points[edge], points[(edge+1)%points.len()]);
        let (dx,dy) = (b.0-a.0,b.1-a.1);
        let length = dx.hypot(dy);
        if length <= f32::EPSILON { continue; }
        let steps = ((length/6.0).ceil() as usize).clamp(8,64);
        let bits = seed.wrapping_add((edge as u64).wrapping_mul(0x9e3779b97f4a7c15));
        let phase = (bits ^ (bits >> 29)).wrapping_mul(0xbf58476d1ce4e5b9) as f64 / u64::MAX as f64;
        let bow = (phase as f32 * 2.0 - 1.0) * 0.8;
        let amplitude = roughness.min(length * 0.04);
        for step in 1..=steps {
            if step == steps { result.push(b); continue; }
            let t = step as f32 / steps as f32;
            let envelope = (std::f32::consts::PI*t).sin();
            let wave = (t*std::f32::consts::TAU*1.5+phase as f32*std::f32::consts::TAU).sin();
            let offset = amplitude * envelope * (bow + wave*0.65);
            result.push((a.0+dx*t-dy/length*offset,a.1+dy*t+dx/length*offset));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curves_keep_vertices_and_depend_on_the_persistent_seed() {
        let vertices = [(0.0,0.0),(120.0,0.0),(120.0,80.0),(0.0,80.0)];
        let a = sketch_outline(&vertices,true,1.2,sketch_seed("a"));
        assert_eq!(a,sketch_outline(&vertices,true,1.2,sketch_seed("a")));
        assert_ne!(a,sketch_outline(&vertices,true,1.2,sketch_seed("b")));
        assert_eq!(a.first(),a.last());
        assert!(vertices.iter().all(|p| a.contains(p)));
        assert!(a.iter().any(|p| p.0>0.0 && p.0<120.0 && p.1.abs()>0.05 && p.1.abs()<3.0));
        assert_eq!(sketch_outline(&vertices,false,0.0,0),vertices);
    }
}
