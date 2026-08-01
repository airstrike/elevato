//! The world canvas: floor strips, elevators, and passengers drawn from
//! the playback's world snapshot in the original's layout geometry
//! (research §3) — floors are 50 px strips, elevators start at x = 200
//! spaced `20 + width` apart with `width = 10 px × capacity` — uniformly
//! scaled down to fit the canvas when the building outgrows it.
//!
//! The [`canvas::Cache`] lives in the app and is cleared on every tick
//! (and on any world-replacing message); between clears the cached
//! geometry is reused. Solid colors only — gradients no-op on wasm.

use iced::widget::{canvas, text};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, alignment, mouse};

use crate::core::World;
use crate::core::elevator::Elevator;
use crate::core::floor;
use crate::playback::Playback;
use crate::theme;

/// Left edge of the first elevator, world px (original layout).
const ELEVATOR_X: f64 = 200.0;

/// Horizontal gap between elevators, world px.
const ELEVATOR_GAP: f64 = 20.0;

/// Width of one passenger slot, world px.
const SLOT_WIDTH: f64 = 10.0;

/// Screen padding around the drawn building, px.
const MARGIN: f32 = 16.0;

/// The canvas program: borrows the playback (whose world it reads only
/// inside `draw`) and the app-owned geometry cache.
pub struct View<'a> {
    playback: &'a Playback,
    cache: &'a canvas::Cache,
}

impl<'a> View<'a> {
    /// Bundles the borrows the canvas draws from.
    pub fn new(playback: &'a Playback, cache: &'a canvas::Cache) -> Self {
        Self { playback, cache }
    }
}

impl<Message> canvas::Program<Message> for View<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let palette = theme::palette(theme);
            frame.fill_rectangle(Point::ORIGIN, frame.size(), palette.canvas_background);
            if let Some(world) = self.playback.world() {
                draw_world(frame, &world, palette);
            }
        });
        vec![geometry]
    }
}

/// A cacheless sibling of [`View`] that borrows a bare [`World`] —
/// previews and tests render frozen worlds without a `Playback` or a
/// long-lived cache (a fresh frame per draw is fine for still frames).
pub struct Still<'a> {
    world: &'a World,
}

impl<'a> Still<'a> {
    /// Wraps a world to draw as-is.
    pub fn new(world: &'a World) -> Self {
        Self { world }
    }
}

impl<Message> canvas::Program<Message> for Still<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let palette = theme::palette(theme);
        frame.fill_rectangle(Point::ORIGIN, frame.size(), palette.canvas_background);
        draw_world(&mut frame, self.world, palette);
        vec![frame.into_geometry()]
    }
}

/// Maps world coordinates (y-down px, floor 0 at the largest y — i.e.
/// at the *bottom* of the screen) into the canvas: a uniform shrink-only
/// scale plus a centering offset.
struct Camera {
    scale: f32,
    offset: iced::Vector,
}

impl Camera {
    /// Fits a `width × height` world into the frame, never upscaling.
    fn fit(frame_size: Size, width: f64, height: f64) -> Self {
        let (width, height) = (width as f32, height as f32);
        let scale = ((frame_size.width - 2.0 * MARGIN) / width)
            .min((frame_size.height - 2.0 * MARGIN) / height)
            .clamp(0.05, 1.0);
        let offset = iced::Vector::new(
            ((frame_size.width - width * scale) / 2.0).max(MARGIN),
            ((frame_size.height - height * scale) / 2.0).max(MARGIN),
        );
        Self { scale, offset }
    }

    fn point(&self, x: f64, y: f64) -> Point {
        Point::new(
            self.offset.x + x as f32 * self.scale,
            self.offset.y + y as f32 * self.scale,
        )
    }

    fn size(&self, width: f64, height: f64) -> Size {
        Size::new(width as f32 * self.scale, height as f32 * self.scale)
    }

    fn px(&self, length: f64) -> f32 {
        length as f32 * self.scale
    }
}

fn draw_world(frame: &mut canvas::Frame, world: &World, palette: theme::Palette) {
    // Elevator left edges: x_{i+1} = x_i + gap + width_i, from x = 200.
    let mut elevator_xs = Vec::with_capacity(world.elevators().len());
    let mut x = ELEVATOR_X;
    for elevator in world.elevators() {
        elevator_xs.push(x);
        x += ELEVATOR_GAP + elevator.capacity() as f64 * SLOT_WIDTH;
    }
    let width = x;
    let height = world.floors().len() as f64 * floor::HEIGHT;
    let camera = Camera::fit(frame.size(), width, height);

    for floor in world.floors() {
        let top = floor.y_position();
        let bottom = top + floor::HEIGHT;

        // The baseline passengers and elevators stand on.
        frame.fill_rectangle(
            camera.point(0.0, bottom - 1.0),
            camera.size(width, 1.0),
            palette.floor_line,
        );

        frame.fill_text(canvas::Text {
            content: floor.level().to_string(),
            position: camera.point(8.0, top + 25.0),
            color: palette.text_secondary,
            size: camera.px(14.0).into(),
            font: theme::MONO,
            align_y: alignment::Vertical::Center,
            ..canvas::Text::default()
        });

        let lit = |on: bool| {
            if on {
                palette.indicator_lit
            } else {
                palette.indicator_unlit
            }
        };
        triangle(
            frame,
            Direction::Up,
            camera.point(34.0, top + 25.0),
            camera.px(4.5),
            lit(floor.up_pressed()),
        );
        triangle(
            frame,
            Direction::Down,
            camera.point(48.0, top + 25.0),
            camera.px(4.5),
            lit(floor.down_pressed()),
        );
    }

    for (elevator, &x) in world.elevators().iter().zip(&elevator_xs) {
        draw_elevator(frame, &camera, elevator, x, palette);
    }

    // Passengers: riders sit in their slots; everyone else queues on
    // their floor's baseline — waiters leftward from the elevators,
    // walk-offs (faded) rightward from the call buttons.
    let floor_count = world.floors().len();
    let mut waiting = vec![0usize; floor_count];
    let mut exiting = vec![0usize; floor_count];
    for passenger in world.passengers() {
        if let (Some(elevator), Some(slot)) = (passenger.aboard(), passenger.slot()) {
            let x = elevator_xs[elevator] + (slot as f64 + 0.5) * SLOT_WIDTH;
            let y = world.elevators()[elevator].y() + floor::HEIGHT - 3.0;
            person(frame, &camera, x, y, palette.passenger);
        } else {
            let level = passenger.current_floor();
            let bottom = world.floors()[level].y_position() + floor::HEIGHT;
            if passenger.is_walking_off() {
                let queued = exiting[level];
                exiting[level] += 1;
                let x = 64.0 + queued as f64 * 11.0;
                person(frame, &camera, x, bottom - 1.0, faded(palette.passenger));
            } else {
                let queued = waiting[level];
                waiting[level] += 1;
                let x = (ELEVATOR_X - 10.0 - queued as f64 * 11.0).max(60.0);
                person(frame, &camera, x, bottom - 1.0, palette.passenger);
            }
        }
    }
}

fn draw_elevator(
    frame: &mut canvas::Frame,
    camera: &Camera,
    elevator: &Elevator,
    x: f64,
    palette: theme::Palette,
) {
    let width = elevator.capacity() as f64 * SLOT_WIDTH;
    let top = elevator.y();

    // The hoist cable, from the cab's roof up out of view.
    frame.fill_rectangle(
        camera.point(x + width / 2.0 - 0.5, 0.0),
        camera.size(1.0, (top + 1.0).max(0.0)),
        palette.floor_line,
    );

    // Cab shell, then the darker header band the lamps sit on.
    frame.fill_rectangle(
        camera.point(x, top + 1.0),
        camera.size(width, floor::HEIGHT - 2.0),
        palette.elevator_body,
    );
    frame.fill_rectangle(
        camera.point(x, top + 1.0),
        camera.size(width, 12.0),
        palette.elevator_band,
    );

    // The door seam splitting the cab face below the band.
    frame.fill_rectangle(
        camera.point(x + width / 2.0 - 0.5, top + 13.0),
        camera.size(1.0, floor::HEIGHT - 15.0),
        palette.elevator_band,
    );

    let lit = |on: bool| {
        if on {
            palette.indicator_lit
        } else {
            palette.indicator_unlit
        }
    };
    triangle(
        frame,
        Direction::Up,
        camera.point(x + 6.0, top + 7.0),
        camera.px(3.0),
        lit(elevator.going_up_indicator()),
    );
    triangle(
        frame,
        Direction::Down,
        camera.point(x + width - 6.0, top + 7.0),
        camera.px(3.0),
        lit(elevator.going_down_indicator()),
    );

    // The floor-position readout, centered between the lamps.
    frame.fill_text(canvas::Text {
        content: elevator.current_floor().to_string(),
        position: camera.point(x + width / 2.0, top + 7.0),
        color: palette.elevator_text,
        size: camera.px(9.0).into(),
        font: theme::MONO,
        align_x: text::Alignment::Center,
        align_y: alignment::Vertical::Center,
        ..canvas::Text::default()
    });

    // Lit destination buttons just under the band, above the riders.
    let dots = ((width - 4.0) / 5.0) as usize;
    for (index, _) in elevator.pressed_floors().into_iter().take(dots).enumerate() {
        let dot = canvas::Path::circle(
            camera.point(x + 4.0 + index as f64 * 5.0, top + 16.5),
            camera.px(1.6),
        );
        frame.fill(&dot, palette.button_lit);
    }
}

/// Which way a triangle indicator points.
enum Direction {
    Up,
    Down,
}

fn triangle(
    frame: &mut canvas::Frame,
    direction: Direction,
    center: Point,
    half: f32,
    color: Color,
) {
    let tip = match direction {
        Direction::Up => -half,
        Direction::Down => half,
    };
    let path = canvas::Path::new(|builder| {
        builder.move_to(Point::new(center.x, center.y + tip));
        builder.line_to(Point::new(center.x - half, center.y - tip));
        builder.line_to(Point::new(center.x + half, center.y - tip));
        builder.close();
    });
    frame.fill(&path, color);
}

/// A simple standing figure — head over body — with its feet at
/// world-space `(x, y)`. No icon art; shapes are enough.
fn person(frame: &mut canvas::Frame, camera: &Camera, x: f64, y: f64, color: Color) {
    let feet = camera.point(x, y);
    frame.fill_rectangle(
        Point::new(feet.x - camera.px(3.0), feet.y - camera.px(9.0)),
        Size::new(camera.px(6.0), camera.px(9.0)),
        color,
    );
    let head = canvas::Path::circle(Point::new(feet.x, feet.y - camera.px(12.0)), camera.px(2.6));
    frame.fill(&head, color);
}

fn faded(color: Color) -> Color {
    color.scale_alpha(0.4)
}

/// Granita previews — native-only dev tooling (plain module `#[cfg]`;
/// the walker cannot see through `cfg_attr`).
#[cfg(not(target_arch = "wasm32"))]
pub mod previews {
    use iced::widget::canvas;
    use iced::{Element, Fill};

    use crate::core::challenge;
    use crate::core::controller::Controller;
    use crate::core::event::Event;
    use crate::core::{World, headless};

    /// A frozen mid-game world: challenge 4 (8 floors, 2 elevators)
    /// driven ~35 sim-seconds by the naive strategy, so the canvas
    /// preview shows real traffic — riders, queues, lit buttons —
    /// instead of an empty lobby. Migrated across reloads.
    pub struct MidGame {
        world: World,
    }

    impl Default for MidGame {
        fn default() -> Self {
            let mut world = World::new(&challenge::roster()[3], 7);
            headless::run(&mut world, &mut Naive, 35 * 60, 1);
            Self { world }
        }
    }

    /// Research §5.1: every elevator serves its own buttons, idles to
    /// the ground floor, and every call button dispatches elevator 0.
    struct Naive;

    impl Controller for Naive {
        fn init(&mut self, _world: &mut World) {}

        fn on_event(&mut self, world: &mut World, event: Event) {
            match event {
                Event::Idle { elevator } => world.go_to_floor(elevator, 0.0, false),
                Event::FloorButtonPressed { elevator, floor } => {
                    world.go_to_floor(elevator, floor as f64, false);
                }
                Event::UpButtonPressed { floor } | Event::DownButtonPressed { floor } => {
                    world.go_to_floor(0, floor as f64, false);
                }
                Event::PassingFloor { .. } | Event::StoppedAtFloor { .. } => {}
            }
        }
    }

    /// Preview: the sim canvas over a busy mid-game world.
    #[granita::preview]
    pub fn mid_game(state: &MidGame) -> Element<'_, crate::app::Message> {
        canvas(super::Still::new(&state.world))
            .width(Fill)
            .height(Fill)
            .into()
    }
}
