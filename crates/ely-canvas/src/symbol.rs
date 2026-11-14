use super::Frame;

/// Non-vendor architecture and diagram symbols.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    /// Cylindrical persistent storage.
    Database,
    /// Cloud service boundary.
    Cloud,
    /// One actor.
    Person,
    /// A group of actors.
    Users,
    /// Physical server rack.
    Server,
    /// Virtual machine in a host.
    VirtualMachine,
    /// Browser window.
    Browser,
    /// Desktop workstation.
    Desktop,
    /// Mobile device.
    Mobile,
    /// Document with a folded corner.
    File,
    /// Folder with a tab.
    Folder,
    /// Object storage bucket.
    ObjectStorage,
    /// Network router.
    Router,
    /// Network switch.
    Switch,
    /// Brick firewall.
    Firewall,
    /// Network gateway.
    Gateway,
    /// Load distribution service.
    LoadBalancer,
    /// Fast cache storage.
    Cache,
    /// Ordered message queue.
    Queue,
    /// Publish/subscribe event bus.
    EventBus,
    /// Component with interface tabs.
    Component,
    /// Circular interface.
    Interface,
    /// Six-sided diagram node.
    Hexagon,
    /// Three-sided diagram node.
    Triangle,
    /// Input/output diagram node.
    Parallelogram,
    /// Named enclosing boundary.
    Container,
    /// Lane with a left-hand title strip.
    SwimlaneHorizontal,
    /// Lane with a top title strip.
    SwimlaneVertical,
}

fn rectangle(x:f32,y:f32,w:f32,h:f32) -> Vec<(f32,f32)> {
    vec![(x,y),(x+w,y),(x+w,y+h),(x,y+h),(x,y)]
}
fn ellipse(x:f32,y:f32,rx:f32,ry:f32) -> Vec<(f32,f32)> {
    (0..=48).map(|i| { let a=std::f32::consts::TAU*i as f32/48.0; (x+rx*a.cos(),y+ry*a.sin()) }).enumerate().map(|(i,p)| if i==48 {(x+rx,y)} else {p}).collect()
}

/// Local-coordinate symbol paths; a repeated first/last point marks a closed path.
/// The first path is the silhouette; remaining paths describe native symbol details.
/// Native rendering and SVG exporters use this same geometry.
pub fn symbol_paths(kind:SymbolKind,width:f32,height:f32) -> Vec<Vec<(f32,f32)>> {
    use SymbolKind::*;
    let rect=||rectangle(0.0,0.0,1.0,1.0);
    let mut paths=match kind {
        Database => {
            let mut body=vec![(0.0,0.16),(0.0,0.84)];
            body.extend((0..=24).map(|i| {let a=std::f32::consts::PI-std::f32::consts::PI*i as f32/24.0;(0.5+0.5*a.cos(),0.84+0.16*a.sin())}));
            body.push((1.0,0.16));
            body.extend((0..=24).map(|i| {let a=-std::f32::consts::PI*i as f32/24.0;(0.5+0.5*a.cos(),0.16+0.16*a.sin())}));
            if let Some(last)=body.last_mut() {*last=(0.0,0.16);}
            vec![body,ellipse(0.5,0.16,0.5,0.16)]
        }
        Cloud => vec![vec![(0.16,0.82),(0.07,0.78),(0.0,0.65),(0.02,0.48),(0.12,0.36),(0.25,0.35),(0.28,0.18),(0.4,0.06),(0.57,0.02),(0.73,0.1),(0.81,0.29),(0.92,0.32),(1.0,0.46),(0.99,0.64),(0.91,0.78),(0.81,0.82),(0.16,0.82)]],
        Person | Users => {
            let mut p=vec![ellipse(0.5,0.16,0.13,0.16),vec![(0.23,0.85),(0.23,0.58),(0.32,0.4),(0.68,0.4),(0.77,0.58),(0.77,0.85),(0.23,0.85)]];
            if kind==Users {p.extend([ellipse(0.15,0.3,0.1,0.12),ellipse(0.85,0.3,0.1,0.12),vec![(0.18,0.48),(0.05,0.53),(0.0,0.85),(0.18,0.85)],vec![(0.82,0.48),(0.95,0.53),(1.0,0.85),(0.82,0.85)]]);}
            p
        }
        Server => vec![rect(),rectangle(0.1,0.08,0.8,0.2),rectangle(0.1,0.37,0.8,0.2),rectangle(0.1,0.66,0.8,0.2),vec![(0.18,0.18),(0.25,0.18)],vec![(0.18,0.47),(0.25,0.47)],vec![(0.18,0.76),(0.25,0.76)]],
        VirtualMachine => vec![rect(),rectangle(0.12,0.12,0.76,0.66),vec![(0.3,0.9),(0.7,0.9)]],
        Browser => vec![rect(),vec![(0.0,0.2),(1.0,0.2)],vec![(0.07,0.1),(0.13,0.1)],vec![(0.19,0.1),(0.25,0.1)],vec![(0.33,0.1),(0.9,0.1)]],
        Desktop => vec![rectangle(0.0,0.0,1.0,0.75),vec![(0.5,0.75),(0.5,1.0)],vec![(0.25,1.0),(0.75,1.0)]],
        Mobile => vec![rectangle(0.18,0.0,0.64,1.0),vec![(0.18,0.12),(0.82,0.12)],vec![(0.18,0.85),(0.82,0.85)],vec![(0.45,0.93),(0.55,0.93)]],
        File => vec![vec![(0.0,0.0),(0.72,0.0),(1.0,0.25),(1.0,1.0),(0.0,1.0),(0.0,0.0)],vec![(0.72,0.0),(0.72,0.25),(1.0,0.25)]],
        Folder => vec![vec![(0.0,0.0),(0.4,0.0),(0.5,0.15),(1.0,0.15),(1.0,1.0),(0.0,1.0),(0.0,0.0)],vec![(0.0,0.28),(1.0,0.28)]],
        ObjectStorage => vec![vec![(0.05,0.12),(0.2,1.0),(0.8,1.0),(0.95,0.12),(0.05,0.12)],ellipse(0.5,0.12,0.45,0.12),rectangle(0.35,0.4,0.3,0.25)],
        Router => vec![ellipse(0.5,0.5,0.5,0.5),vec![(0.15,0.5),(0.85,0.5),(0.7,0.35)],vec![(0.5,0.15),(0.5,0.85),(0.35,0.7)],vec![(0.15,0.5),(0.3,0.65)],vec![(0.5,0.15),(0.65,0.3)]],
        Switch => {let mut p=vec![rect(),vec![(0.1,0.2),(0.9,0.2)]];for i in 0..4 {p.push(rectangle(0.1+i as f32*0.21,0.65,0.15,0.2));}p}
        Firewall => vec![rect(),vec![(0.0,0.25),(1.0,0.25)],vec![(0.0,0.5),(1.0,0.5)],vec![(0.0,0.75),(1.0,0.75)],vec![(0.5,0.0),(0.5,0.25)],vec![(0.25,0.25),(0.25,0.5)],vec![(0.75,0.25),(0.75,0.5)],vec![(0.5,0.5),(0.5,0.75)],vec![(0.25,0.75),(0.25,1.0)],vec![(0.75,0.75),(0.75,1.0)]],
        Gateway => vec![vec![(0.0,1.0),(0.0,0.0),(1.0,0.0),(1.0,1.0),(0.75,1.0),(0.75,0.25),(0.25,0.25),(0.25,1.0),(0.0,1.0)]],
        LoadBalancer => vec![rect(),vec![(0.1,0.5),(0.45,0.5),(0.7,0.2),(0.9,0.2)],vec![(0.45,0.5),(0.9,0.5)],vec![(0.45,0.5),(0.7,0.8),(0.9,0.8)],vec![(0.8,0.1),(0.9,0.2),(0.8,0.3)],vec![(0.8,0.4),(0.9,0.5),(0.8,0.6)],vec![(0.8,0.7),(0.9,0.8),(0.8,0.9)]],
        Cache => vec![rect(),vec![(0.56,0.08),(0.25,0.55),(0.5,0.55),(0.43,0.92),(0.77,0.4),(0.53,0.4),(0.56,0.08)]],
        Queue => vec![rect(),rectangle(0.12,0.27,0.18,0.46),rectangle(0.41,0.27,0.18,0.46),rectangle(0.7,0.27,0.18,0.46)],
        EventBus => vec![rect(),vec![(0.1,0.5),(0.9,0.5)],vec![(0.25,0.2),(0.25,0.8)],vec![(0.5,0.2),(0.5,0.8)],vec![(0.75,0.2),(0.75,0.8)]],
        Component => vec![rectangle(0.12,0.0,0.88,1.0),rectangle(0.0,0.18,0.25,0.2),rectangle(0.0,0.62,0.25,0.2)],
        Interface => vec![ellipse(0.5,0.42,0.32,0.32),vec![(0.5,0.74),(0.5,1.0)]],
        Hexagon => vec![vec![(0.25,0.0),(0.75,0.0),(1.0,0.5),(0.75,1.0),(0.25,1.0),(0.0,0.5),(0.25,0.0)]],
        Triangle => vec![vec![(0.5,0.0),(1.0,1.0),(0.0,1.0),(0.5,0.0)]],
        Parallelogram => vec![vec![(0.2,0.0),(1.0,0.0),(0.8,1.0),(0.0,1.0),(0.2,0.0)]],
        Container | SwimlaneHorizontal | SwimlaneVertical => vec![rect()],
    };
    let icon_height=if matches!(kind,Person | Users | Router | Switch | Firewall | LoadBalancer | Cache | Queue | EventBus | ObjectStorage | Server) {height*0.66} else {height};
    for path in &mut paths {for (x,y) in path { *x*=width;*y*=icon_height; }}
    match kind {
        Container | SwimlaneVertical => paths.push(vec![(0.0,title_height(height)),(width,title_height(height))]),
        SwimlaneHorizontal => paths.push(vec![(title_width(width),0.0),(title_width(width),height)]),
        _ => {}
    }
    paths
}
fn title_height(height:f32)->f32 {(height*0.22).clamp(0.0,32.0)}
fn title_width(width:f32)->f32 {(width*0.22).clamp(0.0,100.0)}

pub(crate) fn label_frame(kind:SymbolKind,w:f32,h:f32)->Frame {
    use SymbolKind::*;
    match kind {
        Container | SwimlaneVertical => Frame::new(8.0,0.0,(w-16.0).max(0.0),title_height(h)),
        SwimlaneHorizontal => Frame::new(4.0,0.0,(title_width(w)-8.0).max(0.0),h),
        Browser | Folder | File | Database => Frame::new(w*0.12,h*0.3,w*0.76,h*0.55),
        Triangle => Frame::new(w*0.25,h*0.5,w*0.5,h*0.4),
        Person | Users | Router | Switch | Firewall | LoadBalancer | Cache | Queue | EventBus | ObjectStorage | Server => Frame::new(w*0.04,h*0.68,w*0.92,h*0.32),
        _ => Frame::new(w*0.2,h*0.28,w*0.6,h*0.44),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn symbol_paths_scale_and_keep_closed_silhouettes() {
        for kind in [SymbolKind::Database,SymbolKind::Cloud,SymbolKind::Hexagon,SymbolKind::Container,SymbolKind::SwimlaneVertical] {
            let a=symbol_paths(kind,100.0,60.0);
            let b=symbol_paths(kind,200.0,120.0);
            assert_eq!(a[0].first(),a[0].last());
            for (a,b) in a[0].iter().zip(&b[0]) {assert_eq!((a.0*2.0,a.1*2.0),*b);}
        }
    }
}
