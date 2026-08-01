//! Divide the available space in two parts to display two different elements.
//
// Originally from iced_aw. https://github.com/iced-rs/iced_aw/
//
// Kept up-to-date with `iced`'s `master` branch by me (`0.14.0-dev`).
//
// New features:
// - Negative divider position, for measuring from the end (right or bottom)

// MIT License

// Copyright (c) 2020 Kaiden42, Andy Terra

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use iced::advanced::layout::{Limits, Node};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Layout, Shell, Widget, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::theme::palette;
use iced::widget::Row;
use iced::{
    Background, Border, Color, Element, Event, Length, Padding, Pixels, Point, Rectangle, Shadow,
    Size, Theme, Vector, border, touch,
};

/// A split can divide the available space by half to display two different elements.
/// It can split horizontally or vertically.
///
/// # Example
/// ```ignore
/// # use iced_aw::split::{State, Axis, Split};
/// # use iced::widget::Text;
/// #
/// #[derive(Debug, Clone)]
/// enum Message {
///     Resized(i16),
/// }
///
/// let first = Text::new("First");
/// let second = Text::new("Second");
///
/// let split = Split::new(first, second, Some(300), Axis::Vertical, Message::Resized);
/// ```
#[allow(missing_debug_implementations)]
pub struct Split<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
    /// The first element of the [`Split`].
    first: Element<'a, Message, Theme, Renderer>,
    /// The second element of the [`Split`].
    second: Element<'a, Message, Theme, Renderer>,
    /// The position of the divider.
    divider_position: Option<Pixels>,
    /// The axis to split at.
    axis: Axis,
    /// The padding around the elements of the [`Split`].
    padding: f32,
    /// The spacing between the elements of the [`Split`].
    /// This is also the width of the divider.
    spacing: f32,
    /// The width of the [`Split`].
    width: Length,
    /// The height of the [`Split`].
    height: Length,
    /// The minimum size of the first element of the [`Split`].
    min_size_first: Pixels,
    /// The minimum size of the second element of the [`Split`].
    min_size_second: Pixels,
    /// The message that is send when the divider of the [`Split`] is moved.
    on_resize: Option<Box<dyn Fn(Pixels) -> Message>>,
    /// The style class of the [`Split`].
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Split<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: 'a + renderer::Renderer,
    Theme: Catalog,
{
    /// Creates a new [`Split`].
    ///
    /// It expects:
    ///     - The first [`Element`] to display
    ///     - The second [`Element`] to display
    ///     - The position of the divider. If none, the space will be split in half.
    ///     - The [`Axis`] to split at.
    ///     - The message that is send on moving the divider
    pub fn new<A, B>(
        first: A,
        second: B,
        divider_position: Option<impl Into<Pixels>>,
        axis: Axis,
    ) -> Self
    where
        A: Into<Element<'a, Message, Theme, Renderer>>,
        B: Into<Element<'a, Message, Theme, Renderer>>,
    {
        Self {
            first: first.into(),
            second: second.into(),
            divider_position: divider_position.map(Into::into),
            axis,
            padding: 0.0,
            spacing: 1.0,
            width: Length::Fill,
            height: Length::Fill,
            min_size_first: 5.into(),
            min_size_second: 5.into(),
            on_resize: None,
            class: Theme::default(),
        }
    }

    /// Sets the on_resize function of the [`Split`].
    pub fn on_resize<F>(mut self, on_resize: F) -> Self
    where
        F: Fn(Pixels) -> Message + 'static,
    {
        self.on_resize = Some(Box::new(on_resize));
        self
    }

    /// Sets the padding of the [`Split`] around the inner elements.
    #[must_use]
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Sets the spacing of the [`Split`] between the elements.
    /// This will also be the width of the divider.
    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Sets the width of the [`Split`].
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Split`].
    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the minimum size of the first element of the [`Split`].
    #[must_use]
    pub fn min_size_first(mut self, size: impl Into<Pixels>) -> Self {
        self.min_size_first = size.into();
        self
    }

    /// Sets the minimum size of the second element of the [`Split`].
    #[must_use]
    pub fn min_size_second(mut self, size: impl Into<Pixels>) -> Self {
        self.min_size_second = size.into();
        self
    }

    /// Sets the style of the [`Split`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`Split`].
    // #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Split<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: 'a + renderer::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut [self.first.as_widget_mut(), self.second.as_widget_mut()]);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let space = Row::<Message, Theme, Renderer>::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .layout(tree, renderer, limits);

        match self.axis {
            Axis::Horizontal => horizontal_split(tree, self, renderer, limits, &space),
            Axis::Vertical => vertical_split(tree, self, renderer, limits, &space),
        }
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        mut cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let split_state: &mut State = state.state.downcast_mut();
        let mut children = layout.children();

        let first_layout = children
            .next()
            .expect("Native: Layout should have a first layout");

        let divider_layout = children
            .next()
            .expect("Native: Layout should have a divider layout");
        if let Some(resize) = self.on_resize.as_ref() {
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
                    if divider_layout
                        .bounds()
                        .expand(5.0)
                        .contains(cursor.position().unwrap_or_default()) =>
                {
                    split_state.dragging = true;
                    cursor = cursor.levitate();
                    shell.capture_event();
                    shell.request_redraw();
                }

                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerLifted { .. })
                    if split_state.dragging =>
                {
                    split_state.dragging = false;
                    cursor = cursor.land();
                    shell.capture_event();
                    shell.request_redraw();
                }

                Event::Mouse(mouse::Event::CursorMoved { position })
                | Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                    if split_state.dragging {
                        let total_size = match self.axis {
                            Axis::Horizontal => layout.bounds().height,
                            Axis::Vertical => layout.bounds().width,
                        };

                        let position = match self.axis {
                            Axis::Horizontal => position.y,
                            Axis::Vertical => position.x,
                        };

                        // Convert the position to i16, taking into account whether we're measuring
                        // from start or end based on current divider position
                        let new_position = if let Some(current_pos) = self.divider_position {
                            if current_pos >= 0.into() {
                                position
                            } else {
                                -(total_size - position)
                            }
                        } else {
                            position
                        };

                        shell.capture_event();
                        shell.publish((resize)(new_position.into()));
                        shell.request_redraw();
                    } else {
                        if cursor.is_levitating() {
                            cursor = cursor.land();
                        }
                    }
                }

                _ => {}
            }
        }

        let second_layout = children
            .next()
            .expect("Native: Layout should have a second layout");

        self.first.as_widget_mut().update(
            &mut state.children[0],
            event,
            first_layout,
            cursor,
            renderer,
            shell,
            viewport,
        );

        self.second.as_widget_mut().update(
            &mut state.children[1],
            event,
            second_layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        let mut children = layout.children();
        let first_layout = children
            .next()
            .expect("Graphics: Layout should have a first layout");
        let divider_layout = children
            .next()
            .expect("Graphics: Layout should have a divider layout");
        let divider_mouse_interaction = if divider_layout
            .bounds()
            .expand(5.0)
            .contains(cursor.position().unwrap_or_default())
        {
            match self.axis {
                Axis::Horizontal => mouse::Interaction::ResizingVertically,
                Axis::Vertical => mouse::Interaction::ResizingHorizontally,
            }
        } else {
            mouse::Interaction::default()
        };

        if state.dragging {
            return divider_mouse_interaction;
        }

        let second_layout = children
            .next()
            .expect("Graphics: Layout should have a second layout");

        let first_mouse_interaction = self.first.as_widget().mouse_interaction(
            &tree.children[0],
            first_layout,
            cursor,
            viewport,
            renderer,
        );
        let second_mouse_interaction = self.second.as_widget().mouse_interaction(
            &tree.children[1],
            second_layout,
            cursor,
            viewport,
            renderer,
        );
        first_mouse_interaction
            .max(second_mouse_interaction)
            .max(divider_mouse_interaction)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        // TODO: clipping!
        let mut children = layout.children();

        let bounds = layout.bounds();
        let content_layout = layout.children().next().unwrap();
        let is_mouse_over = cursor.is_over(bounds);

        let (status, snap) = if is_mouse_over {
            let state = tree.state.downcast_ref::<State>();

            (
                if state.dragging {
                    Status::Dragging
                } else {
                    Status::Hovered
                },
                !state.dragging,
            )
        } else {
            (Status::Active, true)
        };

        let style = theme.style(&self.class, status);

        // Background
        renderer.fill_quad(
            renderer::Quad {
                bounds: content_layout.bounds(),
                border: style.border,
                shadow: Shadow::default(),
                snap,
            },
            style
                .background
                .unwrap_or_else(|| Color::TRANSPARENT.into()),
        );

        let first_layout = children
            .next()
            .expect("Graphics: Layout should have a first layout");

        let bounds_first = first_layout.bounds();
        let is_mouse_over_first = cursor.is_over(bounds_first);

        let status_first = if is_mouse_over_first {
            let state = tree.state.downcast_ref::<State>();

            if state.dragging {
                Status::Dragging
            } else {
                Status::Hovered
            }
        } else {
            Status::Active
        };

        let style_first = theme.style(&self.class, status_first);

        // First
        renderer.fill_quad(
            renderer::Quad {
                bounds: bounds_first,
                border: style_first.first_border,
                shadow: Shadow::default(),
                snap,
            },
            style_first
                .first_background
                .unwrap_or_else(|| Color::TRANSPARENT.into()),
        );

        self.first.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &renderer::Style::default(),
            first_layout,
            cursor,
            viewport,
        );

        let divider_layout = children
            .next()
            .expect("Graphics: Layout should have a divider layout");

        // Second
        let second_layout = children
            .next()
            .expect("Graphics: Layout should have a second layout");

        let bounds_second = second_layout.bounds();
        let is_mouse_over_second = cursor.is_over(bounds_second);

        let status_second = if is_mouse_over_second {
            let state = tree.state.downcast_ref::<State>();

            if state.dragging {
                Status::Dragging
            } else {
                Status::Hovered
            }
        } else {
            Status::Active
        };

        let style_second = theme.style(&self.class, status_second);

        renderer.fill_quad(
            renderer::Quad {
                bounds: bounds_second,
                border: style_second.second_border,
                shadow: Shadow::default(),
                snap,
            },
            style_second
                .second_background
                .unwrap_or_else(|| Color::TRANSPARENT.into()),
        );

        self.second.as_widget().draw(
            &tree.children[1],
            renderer,
            theme,
            &renderer::Style::default(),
            second_layout,
            cursor,
            viewport,
        );

        let bounds_divider = divider_layout.bounds();
        let is_mouse_over_divider = cursor.is_over(bounds_divider.expand(5.0));

        let status_divider = if is_mouse_over_divider {
            let state = tree.state.downcast_ref::<State>();

            if state.dragging {
                Status::Dragging
            } else {
                Status::Hovered
            }
        } else {
            Status::Active
        };

        let style_divider = theme.style(&self.class, status_divider);

        renderer.fill_quad(
            renderer::Quad {
                bounds: divider_layout.bounds(),
                border: style_divider.divider_border,
                shadow: Shadow::default(),
                snap: true,
            },
            style_divider.divider_background,
        );
    }

    fn operate(
        &mut self,
        state: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        let mut children = layout.children();
        let first_layout = children.next().expect("Missing Split First window");
        let _divider_layout = children.next().expect("Missing Split Divider");
        let second_layout = children.next().expect("Missing Split Second window");

        let (first_state, second_state) = state.children.split_at_mut(1);

        self.first
            .as_widget_mut()
            .operate(&mut first_state[0], first_layout, renderer, operation);
        self.second.as_widget_mut().operate(
            &mut second_state[0],
            second_layout,
            renderer,
            operation,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        state: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let mut children = layout.children();
        let first_layout = children.next()?;
        let _divider_layout = children.next()?;
        let second_layout = children.next()?;

        let first = &mut self.first;
        let second = &mut self.second;

        // Not pretty but works to get two mutable references
        // https://stackoverflow.com/a/30075629
        let (first_state, second_state) = state.children.split_at_mut(1);

        first
            .as_widget_mut()
            .overlay(
                &mut first_state[0],
                first_layout,
                renderer,
                viewport,
                translation,
            )
            .or_else(|| {
                second.as_widget_mut().overlay(
                    &mut second_state[0],
                    second_layout,
                    renderer,
                    viewport,
                    translation,
                )
            })
    }
}

/// Do a horizontal split.
fn horizontal_split<'a, Message, Theme, Renderer>(
    tree: &mut Tree,
    split: &mut Split<'a, Message, Theme, Renderer>,
    renderer: &Renderer,
    limits: &Limits,
    space: &Node,
) -> Node
where
    Renderer: 'a + renderer::Renderer,
    Theme: Catalog,
{
    let total_height = space.bounds().height;

    if total_height < split.spacing + f32::from(split.min_size_first + split.min_size_second) {
        return Node::with_children(
            space.bounds().size(),
            vec![
                split.first.as_widget_mut().layout(
                    &mut tree.children[0],
                    renderer,
                    &limits.clone().shrink(Size::new(0.0, total_height)),
                ),
                Node::new(Size::new(space.bounds().width, split.spacing)),
                split.second.as_widget_mut().layout(
                    &mut tree.children[1],
                    renderer,
                    &limits.clone().shrink(Size::new(0.0, total_height)),
                ),
            ],
        );
    }

    let divider_position = split.divider_position.unwrap_or(0.into());
    let effective_position = if divider_position >= 0.into() {
        // Positive: measure from start (top)
        divider_position.0.max(split.spacing / 2.0)
    } else {
        // Negative: measure from end (bottom)
        total_height - (-divider_position.0).max(split.spacing / 2.0)
    };

    // Clamp the effective position to respect minimum sizes
    let clamped_position = effective_position.clamp(
        split.min_size_first.0,
        total_height - split.min_size_second.0 - split.spacing,
    );

    let padding = Padding::from(split.padding as u16);

    // Layout first element
    let first_limits = limits
        .clone()
        .shrink(Size::new(0.0, total_height - clamped_position))
        .shrink(padding);
    let mut first =
        split
            .first
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, &first_limits);
    first.move_to_mut(Point::new(
        space.bounds().x + split.padding,
        space.bounds().y + split.padding,
    ));

    // Layout divider
    let mut divider = Node::new(Size::new(space.bounds().width, split.spacing));
    divider.move_to_mut(Point::new(space.bounds().x, clamped_position));

    // Layout second element
    let second_limits = limits
        .clone()
        .shrink(Size::new(0.0, clamped_position + split.spacing))
        .shrink(padding);
    let mut second =
        split
            .second
            .as_widget_mut()
            .layout(&mut tree.children[1], renderer, &second_limits);
    second.move_to_mut(Point::new(
        space.bounds().x + split.padding,
        space.bounds().y + clamped_position + split.spacing + split.padding,
    ));

    Node::with_children(space.bounds().size(), vec![first, divider, second])
}

/// Do a vertical split.
fn vertical_split<'a, Message, Theme, Renderer>(
    tree: &mut Tree,
    split: &mut Split<'a, Message, Theme, Renderer>,
    renderer: &Renderer,
    limits: &Limits,
    space: &Node,
) -> Node
where
    Renderer: 'a + renderer::Renderer,
    Theme: Catalog,
{
    let total_width = space.bounds().width;

    if total_width < split.spacing + f32::from(split.min_size_first + split.min_size_second) {
        return Node::with_children(
            space.bounds().size(),
            vec![
                split.first.as_widget_mut().layout(
                    &mut tree.children[0],
                    renderer,
                    &limits.clone().shrink(Size::new(total_width, 0.0)),
                ),
                Node::new(Size::new(split.spacing, space.bounds().height)),
                split.second.as_widget_mut().layout(
                    &mut tree.children[1],
                    renderer,
                    &limits.clone().shrink(Size::new(total_width, 0.0)),
                ),
            ],
        );
    }

    let divider_position = split.divider_position.unwrap_or(0.into());
    let effective_position = if divider_position >= 0.into() {
        // Positive: measure from start (left)
        divider_position.0.max(split.spacing / 2.0)
    } else {
        // Negative: measure from end (right)
        total_width - (-divider_position.0).max(split.spacing / 2.0)
    };

    // Clamp the effective position to respect minimum sizes
    let clamped_position = effective_position.clamp(
        split.min_size_first.0,
        total_width - split.min_size_second.0 - split.spacing,
    );

    let padding = Padding::from(split.padding as u16);

    // Layout first element
    let first_limits = limits
        .clone()
        .shrink(Size::new(total_width - clamped_position, 0.0))
        .shrink(padding);
    let mut first =
        split
            .first
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, &first_limits);
    first.move_to_mut(Point::new(
        space.bounds().x + split.padding,
        space.bounds().y + split.padding,
    ));

    // Layout divider
    let mut divider = Node::new(Size::new(split.spacing, space.bounds().height));
    divider.move_to_mut(Point::new(clamped_position, space.bounds().y));

    // Layout second element
    let second_limits = limits
        .clone()
        .shrink(Size::new(clamped_position + split.spacing, 0.0))
        .shrink(padding);
    let mut second =
        split
            .second
            .as_widget_mut()
            .layout(&mut tree.children[1], renderer, &second_limits);
    second.move_to_mut(Point::new(
        space.bounds().x + clamped_position + split.spacing + split.padding,
        space.bounds().y + split.padding,
    ));

    Node::with_children(space.bounds().size(), vec![first, divider, second])
}

impl<'a, Message, Theme, Renderer> From<Split<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: renderer::Renderer + 'a,
    Theme: Catalog + 'a,
{
    fn from(split_pane: Split<'a, Message, Theme, Renderer>) -> Self {
        Element::new(split_pane)
    }
}

/// The state of a [`Split`].
#[derive(Debug, Clone, Default)]
pub struct State {
    /// If the divider is dragged by the user.
    dragging: bool,
}

impl State {
    /// Creates a new [`State`] for a [`Split`].
    ///
    /// It expects:
    ///     - The optional position of the divider. If none, the available space will be split in half.
    ///     - The [`Axis`] to split at.
    #[must_use]
    pub const fn new() -> Self {
        Self { dragging: false }
    }
}

/// The axis to split at.
#[derive(Clone, Copy, Debug, Default)]
pub enum Axis {
    /// Split horizontally.
    Horizontal,
    /// Split vertically.
    #[default]
    Vertical,
}

/// The possible statuses of a [`Split`].
pub enum Status {
    /// The [`Split`] can be dragged.
    Active,
    /// The [`Split`] can be dragged and it is being hovered.
    Hovered,
    /// The [`Split`] is being dragged.
    Dragging,
    /// The [`Split`] cannot be dragged.
    Disabled,
}

/// The style of a [`Split`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The optional background of the [`Split`].
    pub background: Option<Background>,
    /// The optional background of the first element of the [`Split`].
    pub first_background: Option<Background>,
    /// The optional background of the second element of the [`Split`].
    pub second_background: Option<Background>,
    /// The [`Border`] of the [`Split`].
    pub border: Border,
    /// The [`Border`] of the [`Split`].
    pub first_border: Border,
    /// The [`Border`] of the [`Split`].
    pub second_border: Border,
    /// The background of the divider of the [`Split`].
    pub divider_background: Background,
    /// The [`Border`] of the divider of the [`Split`].
    pub divider_border: Border,
}

impl Style {
    /// Updates the [`Style`] with the given [`Background`].
    pub fn with_background(self, background: impl Into<Background>) -> Self {
        Self {
            background: Some(background.into()),
            ..self
        }
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: None,
            first_background: None,
            second_background: None,
            border: Border::default(),
            first_border: Border::default(),
            second_border: Border::default(),
            divider_background: Background::Color(Color::TRANSPARENT),
            divider_border: Border::default(),
        }
    }
}

/// The theme catalog of a [`Split`].
pub trait Catalog {
    /// The item class of the [`Split`].
    type Class<'a>;

    /// The default class produced by the [`Split`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// A styling function for a [`Split`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(base)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

pub fn base(theme: &Theme, status: Status) -> Style {
    let palette = theme.palette();
    let base = styled(palette.primary.strong);

    match status {
        Status::Active => base,
        Status::Hovered => Style {
            background: Some(Background::Color(palette.primary.base.color)),
            divider_background: Background::Color(palette.primary.strong.color),
            ..base
        },
        Status::Dragging => Style {
            background: Some(Background::Color(palette.primary.weak.color)),
            divider_background: Background::Color(palette.primary.base.color),
            ..base
        },
        Status::Disabled => disabled(base),
    }
}

fn styled(pair: palette::Pair) -> Style {
    Style {
        background: Some(Background::Color(pair.color)),
        border: border::rounded(2),
        ..Style::default()
    }
}

fn disabled(style: Style) -> Style {
    Style {
        background: style
            .background
            .map(|background| background.scale_alpha(0.5)),
        ..style
    }
}
