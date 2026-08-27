use puri::{Affine, BezPath, Canvas, Circle, Color, ColorStop, Gradient, Rect};

pub(super) struct Stats {
    pub branches: usize,
    pub blossoms: usize,
}

struct Random(u64);

impl Random {
    fn between(&mut self, minimum: f64, maximum: f64) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let unit = ((self.0 >> 11) as f64) / ((1_u64 << 53) as f64);
        minimum + (maximum - minimum) * unit
    }
}

#[derive(Clone, Copy)]
struct Branch {
    depth: u32,
    angle: f64,
    x: f64,
    y: f64,
    width: f64,
}

pub(super) fn draw<C: Canvas>(
    canvas: &mut C,
    width: f64,
    height: f64,
    outer: Affine,
) -> Stats {
    sky(canvas, width, height, outer);
    mountains(canvas, width, height, outer);
    let (branch_count, blossom_points) = branches(canvas, width, height, outer);
    let blossom_count = blossoms(canvas, &blossom_points, outer);
    Stats {
        branches: branch_count,
        blossoms: blossom_count,
    }
}

fn sky<C: Canvas>(canvas: &mut C, width: f64, height: f64, outer: Affine) {
    let stops = [
        ColorStop {
            offset: 0.0,
            color: Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff).into(),
        },
        ColorStop {
            offset: 1.0,
            color: Color::from_rgba8(0xd3, 0xf8, 0xff, 0xff).into(),
        },
    ];
    canvas.fill(
        Rect::new(0.0, 0.0, width, height),
        Gradient::new_linear((0.0, 0.0), (0.0, height)).with_stops(stops.as_slice()),
        outer,
    );
}

fn mountains<C: Canvas>(canvas: &mut C, width: f64, height: f64, outer: Affine) {
    let mut random = Random(0);
    let scene = Rect::new(0.0, 0.0, width, height);
    mountain(
        canvas,
        &mut random,
        scene,
        130.0,
        Color::from_rgba8(0x8b, 0xb2, 0xbb, 0xff),
        outer,
    );
    mountain(
        canvas,
        &mut random,
        scene,
        50.0,
        Color::from_rgba8(0x61, 0x80, 0x87, 0xff),
        outer,
    );
}

fn mountain<C: Canvas>(
    canvas: &mut C,
    random: &mut Random,
    scene: Rect,
    offset: f64,
    color: Color,
    outer: Affine,
) {
    let mut x = 0.0;
    let mut y = scene.height() - offset;
    let mut path = BezPath::new();
    path.move_to((x, y));
    while x >= 0.0 && x < scene.width() {
        x += random.between(2.0, 10.0);
        y += random.between(-4.0, 3.0);
        path.line_to((x, y));
    }
    path.line_to((scene.width(), scene.height()));
    path.line_to((0.0, scene.height()));
    path.close_path();
    canvas.fill(path, color, outer);
}

fn branches<C: Canvas>(
    canvas: &mut C,
    width: f64,
    height: f64,
    outer: Affine,
) -> (usize, Vec<[f64; 4]>) {
    let mut random = Random(0);
    let mut frontier = vec![Branch {
        depth: 0,
        angle: -std::f64::consts::PI / 2.0,
        x: width / 2.0,
        y: height,
        width: 30.0,
    }];
    let mut branch_count = 0;
    let mut blossom_points = Vec::new();
    while let Some(branch) = frontier.pop() {
        let depth = branch.depth as f64;
        let random_scale = random.between(0.7, 1.3);
        let amount = (depth - 1.0) / (12.0 - 1.0);
        let scaled_length = (60.0 + (3.0 - 60.0) * amount) * random_scale;
        let length = if branch.depth == 0 {
            97.0
        } else {
            scaled_length
        };
        let half_width = branch.width / 2.0;
        let tip_distance = length - half_width;
        let tip_x = branch.x + tip_distance * branch.angle.cos();
        let tip_y = branch.y + tip_distance * branch.angle.sin();
        let child = |angle| Branch {
            depth: branch.depth + 1,
            angle,
            x: tip_x,
            y: tip_y,
            width: branch.width * 0.7,
        };
        if branch.depth < 6 {
            let left = branch.angle + random.between(-0.15, -0.05) * std::f64::consts::PI;
            let right = branch.angle + random.between(0.15, 0.05) * std::f64::consts::PI;
            frontier.push(child(right));
            frontier.push(child(left));
        } else if branch.depth < 12 {
            let left = branch.angle + random.between(0.25, -0.05) * std::f64::consts::PI;
            frontier.push(child(left));
        }
        if branch.depth > 4 {
            blossom_points.push([branch.x, branch.y, tip_x, tip_y]);
        }
        canvas.fill(
            Rect::new(0.0, -half_width, length, half_width),
            Color::BLACK,
            outer * Affine::translate((branch.x, branch.y)) * Affine::rotate(branch.angle),
        );
        branch_count += 1;
    }
    (branch_count, blossom_points)
}

fn blossoms<C: Canvas>(canvas: &mut C, points: &[[f64; 4]], outer: Affine) -> usize {
    let colors = [
        Color::from_rgba8(0xf5, 0xce, 0xea, 0x99),
        Color::from_rgba8(0xe8, 0xd9, 0xe4, 0x99),
        Color::from_rgba8(0xf7, 0xc9, 0xf3, 0x99),
        Color::from_rgba8(0xeb, 0xb4, 0xcc, 0x99),
    ];
    let mut random = Random(0);
    let mut count = 0;
    for [x0, y0, x1, y1] in points {
        for _ in 0..16 {
            let x_amount = random.between(0.0, 1.0);
            let x_jitter = random.between(-10.0, 10.0);
            let y_amount = random.between(0.0, 1.0);
            let y_jitter = random.between(-10.0, 10.0);
            let color = colors[random.between(0.0, colors.len() as f64).floor() as usize];
            let radius = random.between(2.0, 5.0);
            let x = x0 + (x1 - x0) * x_amount + x_jitter;
            let y = y0 + (y1 - y0) * y_amount + y_jitter;
            canvas.fill(Circle::new((x, y), radius), color, outer);
            count += 1;
        }
    }
    count
}
