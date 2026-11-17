#![windows_subsystem = "windows"]

use std::{cmp::Ordering, f32::consts::PI, time::Instant};

use glium::{Display, IndexBuffer, Program, Surface, SwapBuffersError, Texture2d, VertexBuffer, glutin::surface::WindowSurface, implement_vertex, index::PrimitiveType, texture::RawImage2d, uniform, winit::{self, application::ApplicationHandler, event::ElementState, keyboard::KeyCode, window::Window}};
use image::GenericImageView;
use thiserror::Error;

const JOHN_SCALE: f32 = 0.1;
const BUMPER_SCALE: f32 = 0.2;
const BUMPER_ASPECT: f32 = 0.6;
const BUMPER_OFFSET: f32 = 0.9;
const JOHN_PNG: &[u8] = include_bytes!("assets/john.png");

fn main() {
    let ev_loop = winit::event_loop::EventLoop::builder().build().expect("Failed to create event loop");
    let (window, display) = glium::backend::glutin::SimpleWindowBuilder::new().with_inner_size(1200, 800).build(&ev_loop);

    let mut johng = Johng::new(display, window);

    ev_loop.run_app(&mut johng).expect("Error while running event loop")
}

struct Johng {
    display: Display<WindowSurface>,
    window: Window,
    program: Program,
    up_pressed: bool,
    down_pressed: bool,
    bumper_positions: (f32, f32),
    bumper_vertex_buf: VertexBuffer<Vertex>,
    bumper_index_buf: IndexBuffer<u32>,
    john_position: (f32, f32),
    john_velocity: (f32, f32),
    john_texture: Texture2d,
    john_vertex_buf: VertexBuffer<Vertex>,
    john_index_buf: IndexBuffer<u32>,
    aspect_ratio: f32,
    has_failed: bool,
    last_frame_time: Instant,
}

impl Johng {
    pub fn new(display: Display<WindowSurface>, window: Window) -> Johng {

        let john_dyn = image::load_from_memory_with_format(JOHN_PNG, image::ImageFormat::Png).expect("Failed to open john png");
        let john_dim = john_dyn.dimensions();
        let john_raw = RawImage2d::from_raw_rgba_reversed(&john_dyn.into_rgba8(), john_dim);
        let john_texture = Texture2d::new(&display, john_raw).expect("Failed to create john texture");

        let program = glium::Program::from_source(
            &display,
            include_str!("shader.vert"),
            include_str!("shader.frag"),
            None
        ).expect("Failed to compile program");

        let win_size = window.inner_size();
        let aspect_ratio = win_size.width as f32 / win_size.height as f32;

        let (john_verts, john_inds) = square(1.0);
        let john_vertex_buf = VertexBuffer::new(&display, &john_verts).expect("Failed to create vertex buffer");
        let john_index_buf = IndexBuffer::new(&display, PrimitiveType::TrianglesList, &john_inds).expect("Failed to create index buffer");

        let (bumper_verts, bumper_inds) = square(BUMPER_ASPECT);
        let bumper_vertex_buf = VertexBuffer::new(&display, &bumper_verts).expect("Failed to create vertex buffer");
        let bumper_index_buf = IndexBuffer::new(&display, PrimitiveType::TrianglesList, &bumper_inds).expect("Failed to create index buffer");

        Johng {
            display,
            window,
            program,
            up_pressed: false,
            down_pressed: false,
            bumper_positions: (0.0, 0.0),
            bumper_vertex_buf,
            bumper_index_buf,
            john_position: (0.0, 0.0),
            john_velocity: (-0.5, 0.4),
            john_texture,
            john_vertex_buf,
            john_index_buf,
            aspect_ratio,
            has_failed: false,
            last_frame_time: Instant::now()
        }
    }

    fn step(&mut self) {
        let now = Instant::now();
        // seconds
        let dt = (now - self.last_frame_time).as_millis() as f32 / 1_000.0;
        self.last_frame_time = now;
        if self.has_failed { return };

        // process input
        match (self.up_pressed, self.down_pressed) {
            (true, false) => {
                self.bumper_positions.0 += dt * 1.5;
                if self.bumper_positions.0 > (1.0 - BUMPER_SCALE) {
                    self.bumper_positions.0 = 1.0 - BUMPER_SCALE
                }
            }
            (false, true) => {
                self.bumper_positions.0 -= dt * 1.5;
                if self.bumper_positions.0 < - (1.0 - BUMPER_SCALE) {
                    self.bumper_positions.0 = - (1.0 - BUMPER_SCALE)
                }
            }
            _ => {}
        }

        // rudimentary AI player two. if john is on the right side of the
        // screen and moving rightwards, then move the bumper towards john.
        if self.john_position.0 >= 0.0 && self.john_velocity.0 > 0.0 {
            let diff = self.john_position.1 - self.bumper_positions.1;
            let step = diff.abs().max(0.1) * 0.03;
            match diff.partial_cmp(&0.0) {
                Some(Ordering::Less) => {
                    self.bumper_positions.1 -= step;
                    if self.bumper_positions.1 < - (1.0 - BUMPER_SCALE) {
                        self.bumper_positions.1 = - (1.0 - BUMPER_SCALE)
                    }
                },
                Some(Ordering::Greater) => {
                    self.bumper_positions.1 += step;
                    if self.bumper_positions.1 > (1.0 - BUMPER_SCALE) {
                        self.bumper_positions.1 = 1.0 - BUMPER_SCALE
                    }
                },
                _ => {}
            }
        }

        // apply velocity
        self.john_position.0 += self.john_velocity.0 * dt;
        self.john_position.1 += self.john_velocity.1 * dt;

        let john_bottom_edge = self.john_position.1 - JOHN_SCALE;
        let john_top_edge = self.john_position.1 + JOHN_SCALE;
        let john_left_edge = self.john_position.0 - JOHN_SCALE;
        let john_right_edge = self.john_position.0 + JOHN_SCALE;

        fn rotate(vec: (f32, f32), deg: f32) -> (f32, f32) {
            let rad = (deg / 360.0) * 2.0 * PI;
            (
                f32::cos(rad) * vec.0 - f32::sin(rad) * vec.1,
                f32::sin(rad) * vec.0 + f32::cos(rad) * vec.1,
            )
        }

        // left bumper collisions
        let left_bumper_right_edge = (-BUMPER_OFFSET) + (BUMPER_SCALE * BUMPER_ASPECT * 0.5);
        let left_bumper_top_edge = self.bumper_positions.0 + BUMPER_SCALE;
        let left_bumper_bottom_edge = self.bumper_positions.0 - BUMPER_SCALE;
        if john_left_edge <= left_bumper_right_edge
            && john_top_edge >= left_bumper_bottom_edge 
            && john_bottom_edge <= left_bumper_top_edge
        {
            let collision_pos = (self.bumper_positions.0 - self.john_position.1) / BUMPER_SCALE;
            let angle = collision_pos * 20.0;
            self.john_velocity.0 = self.john_velocity.0.abs();
            self.john_velocity = rotate(self.john_velocity, -angle);
            self.john_position.0 = left_bumper_right_edge + JOHN_SCALE + 0.01;
        }

        // right bumper collisions
        let right_bumper_left_edge = BUMPER_OFFSET - (BUMPER_SCALE * BUMPER_ASPECT * 0.5);
        let right_bumper_top_edge = self.bumper_positions.1 + BUMPER_SCALE;
        let right_bumper_bottom_edge = self.bumper_positions.1 - BUMPER_SCALE;
        if john_right_edge >= right_bumper_left_edge
            && john_top_edge >= right_bumper_bottom_edge
            && john_bottom_edge <= right_bumper_top_edge
        {
            self.john_velocity.0 = - self.john_velocity.0.abs();
        }

        // top wall collisions
        if self.john_position.1 >= 1.0 - JOHN_SCALE {
            self.john_position.1 = 1.0 - JOHN_SCALE;
            self.john_velocity.1 = - (self.john_velocity.1.abs());
        // bottom wall collisions
        } else if self.john_position.1 <= - (1.0 - JOHN_SCALE) {
            self.john_position.1 = - (1.0 - JOHN_SCALE);
            self.john_velocity.1 = self.john_velocity.1.abs();
        }

        // left wall collisions
        if self.john_position.0 <= - (1.0 - JOHN_SCALE) {
            self.john_position.0 = - (1.0 - JOHN_SCALE);
            self.john_velocity = (0.0, 0.0);
            self.has_failed = true;
        // right wall collisions
        } else if self.john_position.0 >= 1.0 - JOHN_SCALE {
            self.john_position.0 = 1.0 - JOHN_SCALE;
            self.john_velocity = (0.0, 0.0);
            self.has_failed = true;
        }
    }

    fn draw(&mut self) -> Result<(), JohngDrawError> {
        let mut target = self.display.draw();
        target.clear_color(1.0/256.0, 1.5/256.0, 2.0/256.0, 1.0);

        target.draw(
            &self.john_vertex_buf,
            &self.john_index_buf,
            &self.program,
            &uniform! {
                aspect_ratio: self.aspect_ratio,
                offset: self.john_position,
                scale: (JOHN_SCALE, JOHN_SCALE),
                tex: &self.john_texture
            },
            &Default::default()
        )?;

        target.draw(
            &self.bumper_vertex_buf,
            &self.bumper_index_buf,
            &self.program,
            &uniform! {
                aspect_ratio: self.aspect_ratio,
                offset: (-BUMPER_OFFSET, self.bumper_positions.0),
                scale: (BUMPER_SCALE, BUMPER_SCALE)
            },
            &Default::default()
        )?;

        target.draw(
            &self.bumper_vertex_buf,
            &self.bumper_index_buf,
            &self.program,
            &uniform! {
                aspect_ratio: self.aspect_ratio,
                offset: (BUMPER_OFFSET, self.bumper_positions.1),
                scale: (-BUMPER_SCALE, BUMPER_SCALE)
            },
            &Default::default()
        )?;

        target.finish()?;

        self.window.request_redraw();
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Vertex {
    pub pos: [f32;3],
    pub tex_coords: [f32;2]
}
implement_vertex!(Vertex, pos, tex_coords);

pub fn square(aspect: f32) -> ( [Vertex;4], [u32;6] ) {
    (
        [
            Vertex { pos: [-aspect, -1.0, 0.0], tex_coords: [0.0, 0.0] },
            Vertex { pos: [-aspect,  1.0, 0.0], tex_coords: [0.0, 1.0] },
            Vertex { pos: [ aspect, -1.0, 0.0], tex_coords: [1.0, 0.0] },
            Vertex { pos: [ aspect,  1.0, 0.0], tex_coords: [1.0, 1.0] },
        ],
        [
            0, 1, 3, 0, 3, 2
        ]
    )
}

impl ApplicationHandler for Johng {
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent as WE;
        match event {
            WE::Resized(new_size) => {
                self.aspect_ratio = new_size.width as f32 / new_size.height as f32;
            },
            WE::CloseRequested | WE::Destroyed => event_loop.exit(),
            WE::KeyboardInput { device_id: _, event, is_synthetic: _ } => {
                if event.repeat { return }

                if event.physical_key == KeyCode::ArrowUp {
                    match event.state {
                        ElementState::Pressed => self.up_pressed = true,
                        ElementState::Released => self.up_pressed = false,
                    }
                }

                if event.physical_key == KeyCode::ArrowDown {
                    match event.state {
                        ElementState::Pressed => self.down_pressed = true,
                        ElementState::Released => self.down_pressed = false,
                    }
                }

                if event.physical_key == KeyCode::Escape {
                    event_loop.exit();
                }

                if event.physical_key == KeyCode::KeyR && event.state == ElementState::Pressed {
                    self.bumper_positions = (0.0, 0.0);
                    self.john_position = (0.0, 0.0);
                    self.john_velocity = (-0.4, 0.3);
                    self.has_failed = false;
                }
            },
            WE::RedrawRequested => {
                self.step();
                if let Err(e) = self.draw() {
                    eprintln!("Error while drawing! Error: {e}")
                }
            },
            _ => {}
        }
    }
}

#[derive(Debug, Error)]
enum JohngDrawError {
    #[error("failed to swap buffers")]
    SwapBuffers(#[from] SwapBuffersError),
    #[error("failed to create vertex buffer")]
    VertexBufferCreation(#[from] glium::vertex::BufferCreationError),
    #[error("failed to create index buffer")]
    IndexBufferCreation(#[from] glium::index::BufferCreationError),
    #[error("failed to draw")]
    Draw(#[from] glium::DrawError),
}
