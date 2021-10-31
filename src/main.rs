use tetra::{Context, ContextBuilder, State};
use tetra::graphics::{self, Color, DrawParams};
use tetra::graphics::mesh::{Mesh, GeometryBuilder, ShapeStyle};
use tetra::math::Vec2;
use rand::prelude::*;

const SCALE: f32 = 7.0;
const DX: f32 = -3.75;
const DY: f32 = 0.0;

const POINT_LIFETIME: f32 = 25.0;
const PARTICLE_LIFETIME: f32 = 160.0;
const POINT_RANDOMNESS: f32 = 0.01;
const EPSILON: f32 = 0.01;
const SUBSTEPS: usize = 5;
const POINT_SIZE: f32 = 1.0;
const NOISE: f32 = 0.5;

type Complex = num::complex::Complex<f32>;

fn f(t: usize, mut x: Complex) -> Complex {
    for i in 2..10 {
        x += x.powi(i) * Complex::new(-i as f32, 0.0).exp();
    }
    x
}

fn noise() -> Complex {
    Complex::new(
        (rand::random::<f32>() * 2.0 - 1.0) * NOISE,
        (rand::random::<f32>() * 2.0 - 1.0) * NOISE,
    )
}

struct Particle {
    color: Color,
    position: Complex,
    old_position: Complex,
    lifetime: f32,
    updated: bool,
}

fn sigmoid(x: f32) -> f32 {
    2.0 / (1.0 + (-x).exp()) - 1.0
}

impl Particle {
    fn new(position: Complex) -> Self {
        // let p = f(0, position);
        let p = position;
        Self {
            color: Color::rgb(0.8 + 0.2 * rand::random::<f32>() * sigmoid(p.norm()), 0.45 + 0.2 * rand::random::<f32>() * sigmoid(-p.im), 0.23),
            old_position: position.clone(),
            position,
            lifetime: rand::random::<f32>() * PARTICLE_LIFETIME,
            updated: false,
        }
    }

    fn random() -> Self {
        Self::new(Complex::new(
            (rand::random::<f32>() * 3.0 - 1.5) * SCALE + DX,
            (rand::random::<f32>() * 3.0 - 1.5) * SCALE + DY
        ))
    }
}

struct VectorFieldState {
    particles: Vec<Particle>,

    circle: Option<Mesh>,
    t: usize,
}

impl VectorFieldState {
    fn new() -> Self {
        Self {
            particles: (0..10000).map(|_| Particle::random()).collect(),
            circle: None,
            t: 0,
        }
    }

    fn update_particles(&mut self) {
        self.t += 1;
        for particle in self.particles.iter_mut() {
            particle.updated = true;
            particle.lifetime += 1.0;

            particle.old_position = particle.position;
            for _ in 0..SUBSTEPS {
                let mut z = f(self.t, particle.position);
                z = z / z.norm(); // + noise();
                particle.position += z * (EPSILON / SUBSTEPS as f32);
            }

            let d = f(self.t, particle.position).norm_sqr();
            if particle.lifetime > PARTICLE_LIFETIME || d > 4.0 * SCALE * SCALE || d.is_nan() {
                *particle = Particle::random();
            }
        }
    }
}

impl State for VectorFieldState {
    fn update(&mut self, _ctx: &mut Context) -> tetra::Result {
        self.update_particles();

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> tetra::Result {
        if self.circle.is_none() {
            self.circle = Some(Mesh::circle(ctx, ShapeStyle::Fill, Vec2::new(0.0, 0.0), POINT_SIZE)?);
        }
        let circle = self.circle.as_ref().unwrap();
        let background = Color::rgb(0.08, 0.085, 0.12);
        let width = tetra::window::get_width(ctx);
        let height = tetra::window::get_height(ctx);

        graphics::set_blend_mode(ctx, graphics::BlendMode::Alpha(graphics::BlendAlphaMode::Multiply));
        // graphics::clear(ctx, background);
        let mut background_params: DrawParams = Vec2::new(0.0, 0.0).into();
        background_params.color = background.with_alpha(0.1);
        Mesh::rectangle(ctx, ShapeStyle::Fill, graphics::Rectangle::new(0.0, 0.0, width as f32, height as f32))?
            .draw(ctx, background_params);

        let mut builder = GeometryBuilder::new();

        for particle in self.particles.iter() {
            if !particle.updated {
                continue;
            }
            let x = ((particle.position.re - DX) / SCALE / 2.0 + 0.5) * width as f32;
            let y = ((particle.position.im - DY) / SCALE / 2.0 + 0.5) * height as f32;
            let old_x = ((particle.old_position.re - DX) / SCALE / 2.0 + 0.5) * width as f32;
            let old_y = ((particle.old_position.im - DY) / SCALE / 2.0 + 0.5) * height as f32;

            let mut params: DrawParams = Vec2::new(x, y).into();
            params.color = particle.color;
            circle.draw(ctx, params.clone());
            params.position = Vec2::new(old_x, old_y);
            circle.draw(ctx, params);

            let line = [Vec2::new(x, y), Vec2::new(old_x, old_y)];

            builder.set_color(particle.color);
            builder.polyline(POINT_SIZE * 2.0, &line)?;
        }

        let mesh = builder.build_mesh(ctx)?;

        mesh.draw(ctx, Vec2::new(0.0, 0.0));

        Ok(())
    }
}

fn main() -> tetra::Result {
    ContextBuilder::new("Vector Fields", 1024, 1024).build()?.run(|_| Ok(VectorFieldState::new()))
}
