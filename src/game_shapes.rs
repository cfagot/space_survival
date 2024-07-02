use std::sync::Arc;

use masonry::Affine;
use vello::{
    kurbo::{self, Stroke},
    peniko::Fill,
    Scene,
};
use xilem::Color;

pub fn ship_shape() -> crate::game::Shape {
    let yrad: f64 = 25.0;
    let xrad = 15.0;
    let radius = (yrad * yrad + xrad * xrad).sqrt();

    let mut scene = Scene::new();
    // draw ship
    let mut path = kurbo::BezPath::new();
    path.move_to((0.0, yrad));
    path.line_to((-xrad, -yrad));
    path.line_to((xrad, -yrad));
    path.line_to((0.0, yrad));
    path.close_path();

    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::rgb8(0xff, 0xff, 0xff),
        None,
        &path,
    );
    scene.stroke(
        &Stroke::new(4.0),
        Affine::IDENTITY,
        Color::rgb8(0xff, 0xff, 0xff),
        None,
        &path,
    );

    crate::game::Shape::new(Arc::new(scene), radius)
}

pub fn border_shape(extent: f64) -> crate::game::Shape {
    let border_width = 64.0;
    // half the border width minus a little bit to make collisions look a little better (due to all collision shapes being circles)
    let extent_slack = border_width / 2.0 - 4.0;

    let extent = extent + extent_slack;
    let mut scene = Scene::new();
    let mut path = kurbo::BezPath::new();
    path.move_to((-extent, -extent));
    path.line_to((extent, -extent));
    path.line_to((extent, extent));
    path.line_to((-extent, extent));
    path.line_to((-extent, -extent));
    path.close_path();

    scene.stroke(
        &Stroke::new(border_width),
        Affine::IDENTITY,
        Color::rgb8(0xff, 0x1f, 0x1f),
        None,
        &path,
    );

    let radius = extent * 2.0_f64.sqrt();
    crate::game::Shape::new(Arc::new(scene), radius)
}

fn line_loop_shape(line_loop: &[(f64, f64)], scale: f64) -> (Scene, f64) {
    let mut scene = Scene::new();
    let mut path = kurbo::BezPath::new();
    let start = line_loop[0];
    path.move_to((scale * start.0, scale * start.1));
    for vert in line_loop.into_iter().skip(1) {
        path.line_to((scale * vert.0, scale * vert.1));
    }
    path.line_to((scale * start.0, scale * start.1));
    path.close_path();

    let radius = scale
        * line_loop
            .iter()
            .map(|(x, y)| (x * x + y * y).sqrt())
            .fold(0.0, f64::max);

    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::rgb8(0x7f, 0x7f, 0x7f),
        None,
        &path,
    );
    scene.stroke(
        &Stroke::new(8.0),
        Affine::IDENTITY,
        Color::rgb8(0x8f, 0x8f, 0x8f),
        None,
        &path,
    );

    (scene, radius)
}

pub fn asteroid_shape(num: usize, radius: f64) -> crate::game::Shape {
    // Below are several 20-sided polygons representing asteroids. They were generated from the following spreadsheet:
    // https://docs.google.com/spreadsheets/d/1xR1n7GgObxkecqYXtzoObPnjP1TU0OGz7YYxIOX1x20/edit?usp=sharing

    let verts0 = [
        (1.00, 0.00),
        (1.17, 0.38),
        (0.94, 0.69),
        (0.55, 0.75),
        (0.25, 0.77),
        (0.00, 0.91),
        (-0.26, 0.79),
        (-0.56, 0.77),
        (-0.66, 0.48),
        (-0.91, 0.30),
        (-1.18, 0.00),
        (-1.24, -0.40),
        (-0.93, -0.68),
        (-0.51, -0.70),
        (-0.29, -0.90),
        (0.00, -1.01),
        (0.31, -0.97),
        (0.75, -1.03),
        (1.16, -0.84),
        (1.26, -0.41),
    ];

    let verts1 = [
        (1.00, 0.00),
        (1.13, 0.37),
        (0.88, 0.64),
        (0.74, 1.02),
        (0.38, 1.17),
        (0.00, 1.06),
        (-0.29, 0.91),
        (-0.60, 0.83),
        (-1.02, 0.74),
        (-1.01, 0.33),
        (-1.18, 0.00),
        (-0.91, -0.30),
        (-0.88, -0.64),
        (-0.64, -0.88),
        (-0.34, -1.05),
        (0.00, -1.23),
        (0.30, -0.91),
        (0.67, -0.93),
        (0.90, -0.65),
        (0.91, -0.29),
    ];

    let verts2 = [
        (1.00, 0.00),
        (1.19, 0.39),
        (0.77, 0.56),
        (0.62, 0.86),
        (0.38, 1.17),
        (0.00, 0.99),
        (-0.23, 0.72),
        (-0.45, 0.62),
        (-0.78, 0.57),
        (-0.61, 0.20),
        (-0.79, 0.00),
        (-0.79, -0.26),
        (-0.47, -0.35),
        (-0.46, -0.64),
        (-0.33, -1.00),
        (0.00, -1.08),
        (0.31, -0.97),
        (0.47, -0.64),
        (0.85, -0.62),
        (0.84, -0.27),
    ];

    let verts3 = [
        (1.00, 0.00),
        (1.03, 0.33),
        (1.02, 0.74),
        (0.63, 0.86),
        (0.33, 1.01),
        (0.00, 0.81),
        (-0.32, 0.98),
        (-0.73, 1.01),
        (-0.97, 0.70),
        (-1.00, 0.33),
        (-0.78, 0.00),
        (-0.62, -0.20),
        (-0.61, -0.45),
        (-0.51, -0.70),
        (-0.30, -0.91),
        (0.00, -0.86),
        (0.32, -0.97),
        (0.58, -0.80),
        (0.91, -0.66),
        (0.89, -0.29),
    ];

    let verts4 = [
        (1.00, 0.00),
        (0.89, 0.29),
        (0.82, 0.60),
        (0.60, 0.82),
        (0.23, 0.70),
        (0.00, 0.84),
        (-0.31, 0.96),
        (-0.45, 0.62),
        (-0.66, 0.48),
        (-0.95, 0.31),
        (-0.96, 0.00),
        (-1.16, -0.38),
        (-1.02, -0.74),
        (-0.61, -0.83),
        (-0.28, -0.85),
        (0.00, -0.86),
        (0.32, -0.98),
        (0.68, -0.94),
        (0.76, -0.55),
        (0.84, -0.27),
    ];

    let verts5 = [
        (1.00, 0.00),
        (1.19, 0.39),
        (0.77, 0.56),
        (0.70, 0.97),
        (0.41, 1.27),
        (0.00, 1.08),
        (-0.42, 1.29),
        (-0.78, 1.07),
        (-1.13, 0.82),
        (-1.27, 0.41),
        (-1.20, 0.00),
        (-1.35, -0.44),
        (-1.05, -0.76),
        (-0.68, -0.93),
        (-0.33, -1.02),
        (0.00, -1.15),
        (0.40, -1.23),
        (0.66, -0.90),
        (1.07, -0.77),
        (1.23, -0.40),
    ];

    let verts = match num % 6 {
        0 => &verts0,
        1 => &verts1,
        2 => &verts2,
        3 => &verts3,
        4 => &verts4,
        5 => &verts5,
        _ => &verts0,
    };

    let (shape, outer_radius) = line_loop_shape(verts, radius);

    crate::game::Shape::new(Arc::new(shape), outer_radius)
}

pub fn air_pod_scene(t: f64) -> Scene {
    let mut scene = Scene::new();
    let mut path = kurbo::BezPath::new();
    let radius = 100.0;

    // t -> 0..1
    let t = t - t.floor();
    // t cycles at 0.0/1.0 and reaches other extreme at 0.5
    let t = (t - 0.5).abs() * 2.0;
    let xscale = t.max(0.25);
    let yscale = (1.0 - t).max(0.25);

    path.move_to((0.0, yscale * -radius));
    path.quad_to((0.0, 0.0), (xscale * radius, 0.0));
    path.quad_to((0.0, 0.0), (0.0, yscale * radius));
    path.quad_to((0.0, 0.0), (xscale * -radius, 0.0));
    path.quad_to((0.0, 0.0), (0.0, yscale * -radius));
    path.close_path();

    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::rgb8(0x0, 0xb4, 0xd8),
        None,
        &path,
    );
    scene.stroke(
        &Stroke::new(2.0),
        Affine::IDENTITY,
        Color::rgb8(0xff, 0xff, 0xff),
        None,
        &path,
    );
    scene
}

pub fn air_pod_shape(t: f64) -> crate::game::Shape {
    let radius = 100.0;
    crate::game::Shape::new(Arc::new(air_pod_scene(t)), radius)
}

pub fn flame_scene(t: f64) -> Scene {
    let mut scene = Scene::new();

    let t = 20.0 * t;

    let t1 = (t.sin() + 0.5 * (2.0 * t).sin() + 0.25 * (4.0 * t).sin()) / 1.75;
    let t2 = (t.cos() + 0.5 * (2.0 * t).cos() + 0.25 * (4.0 * t).sin()) / 1.75;
    let t3 = ((1.0 + t).sin() + 0.5 * (0.3 + 2.0 * t).sin() + 0.25 * (2.0 + 4.0 * t).sin()) / 1.75;
    let t4 = ((1.0 + t).cos() + 0.5 * (0.7 + 2.0 * t).cos() + 0.25 * (1.7 + 4.0 * t).sin()) / 1.75;

    // keep everything 0..1
    let t1 = 0.1 + (t1 + 1.0) / 2.0;
    let t2 = 0.1 + (t2 + 1.0) / 2.0;
    let t3 = 0.1 + (t3 + 1.0) / 2.0;
    let t4 = 0.1 + (t4 + 1.0) / 2.0;

    let mut create_flame = |x_base1, x_base2, x_tip, y_base, y_tip, t| {
        let mut path = kurbo::BezPath::new();
        let yd = y_tip - y_base;
        let xd1 = x_tip - x_base1;
        let xd2 = x_base2 - x_tip;

        path.move_to((x_base1, y_base));
        path.quad_to(
            (x_base1 + 0.5 * xd1, y_base + 0.1 * yd * t),
            (x_base1 + xd1, y_base + yd * t),
        );
        path.quad_to(
            (x_tip + 0.1 * xd2, y_base + 0.5 * yd * t),
            (x_base2, y_base),
        );
        path.line_to((x_base1, y_base));

        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::rgb8(0xcf, 0x00, 0x00),
            None,
            &path,
        );
        scene.stroke(
            &Stroke::new(2.0),
            Affine::IDENTITY,
            Color::rgb8(0xff, 0xa5, 0x00),
            None,
            &path,
        );
    };

    create_flame(14.0, 0.0, 10.0, -25.0, -39.5, t1);
    create_flame(-14.0, 0.0, -10.0, -25.0, -40.5, t2);
    create_flame(-12.5, 7.5, -2.5, -25.0, -54.5, t3);
    create_flame(12.5, -7.5, 2.5, -25.0, -55.5, t4);
    // create_flame( 28.0, 0.0, 20.0, -50.0, -79.0, t1);
    // create_flame( -28.0, 0.0, -20.0, -50.0, -81.0, t2);
    // create_flame(-25.0, 15.0, -5.0, -50.0, -109.0, t3);
    // create_flame( 25.0, -15.0, 5.0, -50.0, -111.0, t4);

    scene
}
