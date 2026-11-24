use super::FillStyle;

/// Returns seeded interior strokes clipped to a closed polygon in canvas coordinates.
pub fn fill_pattern_segments(points: &[(f32, f32)], style: FillStyle, seed: u64, spacing: f32) -> Vec<[(f32, f32); 2]> {
    if points.len() < 3 || style == FillStyle::Solid { return Vec::new(); }
    let spacing = spacing.max(1.0);
    let mut result = Vec::new();
    let phase = (seed % 997) as f32 / 997.0 * spacing;
    let passes = if style == FillStyle::CrossHatch { 2 } else { 1 };
    for pass in 0..passes {
        let sign = if pass == 0 { 1.0 } else { -1.0 };
        let rotate = |p: (f32, f32)| ((p.0 + sign * p.1) * std::f32::consts::FRAC_1_SQRT_2, (p.1 - sign * p.0) * std::f32::consts::FRAC_1_SQRT_2);
        let restore = |p: (f32, f32)| ((p.0 - sign * p.1) * std::f32::consts::FRAC_1_SQRT_2, (p.1 + sign * p.0) * std::f32::consts::FRAC_1_SQRT_2);
        let polygon: Vec<_> = points.iter().copied().map(rotate).collect();
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
        for &(x,y) in &polygon { min_x=min_x.min(x); max_x=max_x.max(x); min_y=min_y.min(y); max_y=max_y.max(y); }
        let mut y = (min_y / spacing).floor() * spacing + phase;
        let mut intersections = Vec::new();
        while y <= max_y {
            if style == FillStyle::Zigzag {
                let mut x = (min_x / spacing).floor() * spacing;
                let mut up = false;
                while x < max_x {
                    let a = (x, y + if up { spacing * 0.5 } else { 0.0 });
                    let b = (x + spacing, y + if up { 0.0 } else { spacing * 0.5 });
                    clipped(a, b, &polygon, &mut intersections, |a,b| result.push([restore(a),restore(b)]));
                    x += spacing;
                    up = !up;
                }
            } else {
                clipped((min_x-spacing,y), (max_x+spacing,y), &polygon, &mut intersections, |a,b| result.push([restore(a),restore(b)]));
            }
            y += spacing;
        }
    }
    result
}

fn clipped(a: (f32,f32), b: (f32,f32), polygon: &[(f32,f32)], cuts: &mut Vec<f32>, mut emit: impl FnMut((f32,f32),(f32,f32))) {
    cuts.clear();
    cuts.extend([0.0,1.0]);
    let d = (b.0-a.0,b.1-a.1);
    let cross = |u:(f32,f32),v:(f32,f32)| u.0*v.1-u.1*v.0;
    for (index,&p) in polygon.iter().enumerate() {
        let q=polygon[(index+1)%polygon.len()];
        let e=(q.0-p.0,q.1-p.1);
        let denom=cross(d,e);
        if denom.abs() <= f32::EPSILON { continue; }
        let offset=(p.0-a.0,p.1-a.1);
        let t=cross(offset,e)/denom;
        let u=cross(offset,d)/denom;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) { cuts.push(t); }
    }
    cuts.sort_by(f32::total_cmp);
    let at=|t:f32| (a.0+d.0*t,a.1+d.1*t);
    for cut in cuts.windows(2) {
        if cut[1]-cut[0] <= f32::EPSILON { continue; }
        let mid=at((cut[0]+cut[1])*0.5);
        let mut inside=false;
        for (index,&p) in polygon.iter().enumerate() {
            let q=polygon[(index+1)%polygon.len()];
            if (p.1>mid.1)!=(q.1>mid.1) && mid.0<(q.0-p.0)*(mid.1-p.1)/(q.1-p.1)+p.0 { inside=!inside; }
        }
        if inside { emit(at(cut[0]),at(cut[1])); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_are_seeded_and_clipped_to_a_concave_polygon() {
        let polygon=[(0.0,0.0),(40.0,0.0),(40.0,10.0),(10.0,10.0),(10.0,40.0),(0.0,40.0)];
        for style in [FillStyle::Hachure,FillStyle::CrossHatch,FillStyle::Zigzag] {
            let lines=fill_pattern_segments(&polygon,style,23,6.0);
            assert!(!lines.is_empty());
            assert_eq!(lines,fill_pattern_segments(&polygon,style,23,6.0));
            assert_ne!(lines,fill_pattern_segments(&polygon,style,24,6.0));
            for [a,b] in lines {
                let mid=((a.0+b.0)*0.5,(a.1+b.1)*0.5);
                assert!(mid.0>=-0.001 && mid.1>=-0.001 && mid.0<=40.001 && mid.1<=40.001);
                assert!(mid.0<=10.001 || mid.1<=10.001,"segment escaped the concavity: {a:?} {b:?}");
            }
        }
    }
}
