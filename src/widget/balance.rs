//! Balances wrapped text like CSS `text-wrap: balance`. Uses the narrowest
//! width that preserves the full-width line count, avoiding a short final
//! line without adding another line.

use iced::advanced::layout::Limits;
use iced::advanced::widget::{Meta, Tree};
use iced::advanced::{Layout, Widget, renderer};
use iced::mouse::Cursor;
use iced::{Length, Rectangle, Size};

/// How close, in logical pixels, the search comes to the narrowest width.
const PRECISION: f32 = 1.0;

/// Wraps `content`, a [`Length::Shrink`] text, balancing its lines.
pub fn balance<W>(content: W) -> Balance<W> {
    Balance { content }
}

/// See [`balance`].
#[derive(Debug)]
pub struct Balance<W> {
    content: W,
}

impl<W> Meta for Balance<W> {}

impl<Message, Theme, Renderer, W> Widget<Message, Theme, Renderer> for Balance<W>
where
    Renderer: renderer::Renderer,
    W: Widget<Message, Theme, Renderer>,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let content = &mut self.content;
        let child = &mut tree.children[0];
        let within = |width: f32| Limits {
            min: Size::ZERO,
            max: Size::new(width, limits.max.height),
            ..*limits
        };

        // Text's height counts its lines: a width is wide enough while the
        // text stays as tall as it is at the full width.
        content.layout(child, renderer, limits);
        let full = child.size;
        let (mut narrow, mut wide) = (0.0, full.width);
        while wide - narrow > PRECISION {
            let width = (narrow + wide) / 2.0;
            content.layout(child, renderer, &within(width));
            if child.size.height > full.height {
                narrow = width;
            } else {
                wide = width;
            }
        }
        // The last layout is the one the content draws with.
        content.layout(child, renderer, &within(wide));
        tree.size = child.size;
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.content.draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }
}
