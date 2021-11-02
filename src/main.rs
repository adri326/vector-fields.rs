use image::RgbaImage;
use rand::prelude::*;
use tetra::{Context, ContextBuilder, State};
use tetra::graphics::{self, Canvas, Color, DrawParams, Shader, ImageData};
use tetra::graphics::mesh::{Mesh, GeometryBuilder, ShapeStyle};
use tetra::math::Vec2;
use scoped_threadpool::Pool;
use std::sync::mpsc::{Receiver, Sender, self};
use std::sync::Mutex;
use std::thread;

const SCALE: f32 = 5.0;
const DX: f32 = -3.75;
const DY: f32 = 0.0;

const PARTICLE_LIFETIME: f32 = 160.0;
const EPSILON: f32 = 0.01;
const SUBSTEPS: usize = 6;
const NOISE: f32 = 0.5;
const PARTICLE_FADE_IN: f32 = 6.0;
const PARTICLE_FADE_OUT: f32 = 6.0;
const N_PARTICLES: usize = 100000;

const POINT_SIZE: f32 = 0.5;
const WIDTH: u32 = 1920 * 1;
const HEIGHT: u32 = 1080 * 1;
const SAVING: bool = true;
const THREADS: u32 = 16;
const TASK_SIZE: usize = 1000;

const LOOP_FRAMES: u32 = 1; // Set to 1 for infinite animation, set to some other value for a looping animation
const PARTICLES_PER_FRAME: u32 = 10000;

type Complex = num::complex::Complex<f32>;

fn f(_t: usize, mut x: Complex) -> Complex {
    for i in 2..12 {
        x += x.powi(i) * Complex::new(-i as f32, 0.0).exp();
    }
    x
}

#[allow(dead_code)]
fn noise() -> Complex {
    Complex::new(
        (rand::random::<f32>() * 2.0 - 1.0) * NOISE,
        (rand::random::<f32>() * 2.0 - 1.0) * NOISE,
    )
}

#[derive(Clone, Copy, Debug)]
struct Particle {
    color: Color,
    position: Complex,
    old_position: Complex,
    lifetime: f32,
    age: f32,
    updated: bool,
}

fn sigmoid(x: f32) -> f32 {
    2.0 / (1.0 + (-x).exp()) - 1.0
}

impl Particle {
    fn new(position: Complex) -> Self {
        let p = f(0, position);
        let mut color = Color::rgb(0.8 + 0.2 * rand::random::<f32>() * sigmoid(p.norm()), 0.45 + 0.2 * rand::random::<f32>() * sigmoid(-p.im), 0.23);
        if rand::random::<f32>() < 0.3 {
            color = Color::rgb(0.08, 0.085, 0.12);
        }
        // let lifetime = LOOP_FRAMES as f32 / (if rand::random::<f32>() < 0.3 {4.0f32} else {8.0f32}).ceil();
        let lifetime = rand::random::<f32>() * PARTICLE_LIFETIME;
        Self {
            color,
            old_position: position.clone(),
            position,
            lifetime,
            age: rand::random::<f32>() * lifetime,
            updated: false,
        }
    }

    fn random(mut t: u32, n: u32) -> Self {
        if LOOP_FRAMES > 1 {
            t %= LOOP_FRAMES;
        }
        let seed: u64 = ((t as u64) << 32) | n as u64;
        let mut r = rand::rngs::StdRng::seed_from_u64(seed);
        let position = Complex::new(
            (r.gen::<f32>() * 3.0 - 1.5) * SCALE * WIDTH.max(HEIGHT) as f32 / WIDTH as f32 + DX,
            (r.gen::<f32>() * 3.0 - 1.5) * SCALE * WIDTH.max(HEIGHT) as f32 / HEIGHT as f32 + DY
        );
        let p = f(t as usize, position);
        let mut color = Color::rgb(0.8 + 0.2 * r.gen::<f32>() * sigmoid(p.norm()), 0.45 + 0.2 * r.gen::<f32>() * sigmoid(-p.im), 0.23);
        if r.gen::<f32>() < 0.3 {
            color = Color::rgb(0.08, 0.085, 0.12);
        }
        let lifetime = r.gen::<f32>() * PARTICLE_LIFETIME;
        Self {
            color,
            old_position: position.clone(),
            position,
            lifetime,
            age: r.gen::<f32>() * lifetime,
            updated: false,
        }
    }
}

struct VectorFieldState {
    particles: Vec<Particle>,

    circle: Option<Mesh>,
    t: usize,

    canvas: Canvas,
    canvas_blur: Canvas,
    canvas_bloom: Canvas,
    shader_blur: Shader,
    shader_bloom: Shader,

    image_tx: Sender<ImageData>,
}

impl VectorFieldState {
    fn new(ctx: &mut Context, image_tx: Sender<ImageData>) -> Self {
        Self {
            particles: (0..N_PARTICLES).map(|n| Particle::random(0, n as u32)).collect(),
            circle: None,
            t: 0,
            canvas: Canvas::new(ctx, WIDTH as i32, HEIGHT as i32).unwrap(),
            canvas_blur: Canvas::new(ctx, WIDTH as i32, HEIGHT as i32).unwrap(),
            canvas_bloom: Canvas::new(ctx, WIDTH as i32, HEIGHT as i32).unwrap(),
            shader_blur: Shader::from_fragment_file(ctx, "shader/blur.frag").unwrap(),
            shader_bloom: Shader::from_fragment_file(ctx, "shader/bloom.frag").unwrap(),

            image_tx,
        }
    }

    fn update_particles(&mut self) {
        let mut pool = Pool::new(THREADS);

        let res: Vec<Particle> = Vec::with_capacity(self.particles.len());
        let res = Mutex::new(res);

        pool.scoped(|scope| {
            let res = &res;
            let particles = &self.particles;
            for n in 0..(particles.len() / TASK_SIZE) {
                let t = self.t;
                scope.execute(move || { // move task_buffer
                    let n = n * TASK_SIZE;
                    let mut task_buffer = Vec::with_capacity(TASK_SIZE);
                    for o in n..(n+TASK_SIZE) {
                        if o >= particles.len() {
                            break;
                        }
                        let mut particle = particles[o].clone();

                        particle.updated = true;
                        particle.age += 1.0;

                        for _ in 0..SUBSTEPS {
                            let mut z = f(t, particle.position);
                            z = z / z.norm(); // + noise();
                            particle.position += z * (EPSILON / SUBSTEPS as f32);
                        }

                        let d = f(t, particle.position).norm_sqr();
                        if !(particle.age >= particle.lifetime || d >= 4.0 * SCALE * SCALE || d.is_nan()) {
                            task_buffer.push(particle);
                        }
                    }

                    match res.lock() {
                        Ok(mut lock) => {
                            lock.append(&mut task_buffer);
                        },
                        Err(err) => panic!("Couldn't lock result buffer! {}", err),
                    }
                });
            }
        });

        let res = res.into_inner().unwrap();
        self.particles = res;
        for n in 0..PARTICLES_PER_FRAME {
            self.particles.push(Particle::random(self.t as u32, n));
        }

        // for particle in self.particles.iter_mut() {
        //     particle.updated = true;
        //     particle.age += 1.0;

        //     for _ in 0..SUBSTEPS {
        //         let mut z = f(self.t, particle.position);
        //         z = z / z.norm(); // + noise();
        //         particle.position += z * (EPSILON / SUBSTEPS as f32);
        //     }

        //     let d = f(self.t, particle.position).norm_sqr();
        //     if particle.age >= particle.lifetime || d > 4.0 * SCALE * SCALE || d.is_nan() {
        //         *particle = Particle::random();
        //     }
        // }
    }
}

impl State for VectorFieldState {
    fn update(&mut self, ctx: &mut Context) -> tetra::Result {
        if LOOP_FRAMES > 1 && self.t > 2 * LOOP_FRAMES as usize {
            println!("Rendering done!");
            tetra::window::quit(ctx);
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> tetra::Result {
        self.update_particles();
        self.t += 1;
        if self.circle.is_none() {
            self.circle = Some(Mesh::circle(ctx, ShapeStyle::Fill, Vec2::new(0.0, 0.0), POINT_SIZE)?);
        }
        let circle = self.circle.as_ref().unwrap();
        let background = Color::rgb(0.08, 0.085, 0.12);
        let width = tetra::window::get_width(ctx);
        let height = tetra::window::get_height(ctx);
        let wh = width.min(height);
        let dx = (width - wh) as f32 / 2.0;
        let dy = (height - wh) as f32 / 2.0;

        graphics::set_blend_mode(ctx, graphics::BlendMode::Alpha(graphics::BlendAlphaMode::Multiply));
        self.canvas.draw(ctx, Vec2::zero());
        graphics::set_canvas(ctx, &self.canvas);
        if self.t <= 1 {
            graphics::clear(ctx, background);
        }
        let mut background_params: DrawParams = Vec2::new(0.0, 0.0).into();
        background_params.color = background.with_alpha(0.07);
        Mesh::rectangle(ctx, ShapeStyle::Fill, graphics::Rectangle::new(0.0, 0.0, width as f32, height as f32))?
            .draw(ctx, background_params);

        let mut builder = GeometryBuilder::new();

        for particle in self.particles.iter_mut() {
            if !particle.updated {
                continue;
            }
            let x = ((particle.position.re - DX) / SCALE / 2.0 + 0.5) * wh as f32 + dx;
            let y = ((particle.position.im - DY) / SCALE / 2.0 + 0.5) * wh as f32 + dy;
            let old_x = ((particle.old_position.re - DX) / SCALE / 2.0 + 0.5) * wh as f32 + dx;
            let old_y = ((particle.old_position.im - DY) / SCALE / 2.0 + 0.5) * wh as f32 + dy;
            particle.old_position = particle.position;

            let mut params: DrawParams = Vec2::new(x, y).into();
            let s = sigmoid(particle.age / PARTICLE_FADE_IN) * sigmoid((particle.lifetime - particle.age) / PARTICLE_FADE_OUT);
            // params.color = particle.color.with_alpha(s);
            // circle.draw(ctx, params.clone());
            // params.position = Vec2::new(old_x, old_y);
            // circle.draw(ctx, params);

            let line = [Vec2::new(x, y), Vec2::new(old_x, old_y)];

            builder.set_color(particle.color.with_alpha(s));
            builder.polyline(POINT_SIZE * 2.0, &line)?;
        }

        let mesh = builder.build_mesh(ctx)?;

        mesh.draw(ctx, Vec2::new(0.0, 0.0));
        graphics::reset_canvas(ctx);

        // Bloom filter, using only 3 frag shaders
        graphics::set_shader(ctx, &self.shader_bloom);
        self.shader_bloom.set_uniform(ctx, "u_threshold", 0.3);
        graphics::set_canvas(ctx, &self.canvas_bloom);

        self.canvas.draw(ctx, Vec2::zero());

        graphics::reset_canvas(ctx);
        graphics::reset_shader(ctx);

        graphics::set_canvas(ctx, &self.canvas_blur);
        graphics::set_shader(ctx, &self.shader_blur);
        self.shader_blur.set_uniform(ctx, "u_stepsize", Vec2::new(1.0 / WIDTH as f32, 1.0 / HEIGHT as f32));
        self.shader_blur.set_uniform(ctx, "u_horizontal", 1i32);
        self.canvas_bloom.draw(ctx, Vec2::zero());

        graphics::reset_canvas(ctx);
        graphics::set_canvas(ctx, &self.canvas_bloom);
        self.shader_blur.set_uniform(ctx, "u_horizontal", 0i32);
        self.canvas_blur.draw(ctx, Vec2::zero());

        graphics::reset_shader(ctx);
        graphics::set_blend_mode(ctx, graphics::BlendMode::Add(graphics::BlendAlphaMode::Multiply));
        self.canvas.draw(ctx, Vec2::zero());
        graphics::reset_canvas(ctx);
        graphics::set_blend_mode(ctx, graphics::BlendMode::Alpha(graphics::BlendAlphaMode::Multiply));
        self.canvas_bloom.draw(ctx, Vec2::zero());

        let image_data = self.canvas_bloom.get_data(ctx);

        if LOOP_FRAMES <= 1 {
            // Print every frame
            self.image_tx.send(image_data).unwrap();
        } else {
            // Only print [LOOP_FRAMES; 2*LOOP_FRAMES[, exit after that
            if self.t >= LOOP_FRAMES as usize && self.t < 2 * LOOP_FRAMES as usize {
                self.image_tx.send(image_data).unwrap();
            }
        }

        Ok(())
    }
}

fn main() -> tetra::Result {
    let (tx, rx): (Sender<ImageData>, Receiver<ImageData>) = mpsc::channel();

    thread::spawn(move || {
        let mut n: usize = 0;
        for image_data in rx {
            n += 1;
            let width = image_data.width() as u32;
            let height = image_data.height() as u32;
            let buffer: RgbaImage = RgbaImage::from_raw(width, height, image_data.into_bytes()).unwrap();
            if SAVING {
                buffer.save(format!("output/{}.png", n)).unwrap();
            }
        }
    });
    ContextBuilder::new("Vector Fields", WIDTH as i32, HEIGHT as i32).build()?.run(|ctx| Ok(VectorFieldState::new(ctx, tx)))
}
