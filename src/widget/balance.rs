//! A wrapper that balances the lines of wrapped text, as CSS's
//! `text-wrap: balance` does: it lays its content out in the narrowest
//! width that still takes as few lines as the full width does, so a
//! wrapped sentence breaks into lines of about one length instead of a
//! long line and a stray last word.

use iced::advanced::layout::{Limits, Node};
use iced::advanced::widget::Tree;
use iced::advanced::{Layout, Widget, renderer};
use iced::mouse::Cursor;
use iced::{Element, Length, Rectangle, Size};

/// How close, in logical pixels, the search comes to the narrowest width.
const PRECISION: f32 = 1.0;

/// Wraps `content`, a [`Length::Shrink`] text, balancing its lines.
pub fn balance<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Balance<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    Balance {
        content: content.into(),
    }
}

/// See [`balance`].
#[allow(missing_debug_implementations)]
pub struct Balance<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: renderer::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Balance<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let content = self.content.as_widget_mut();
        let tree = &mut tree.children[0];
        let max = limits.max();
        let within = |width: f32| Limits::new(Size::ZERO, Size::new(width, max.height));

        // Text's height counts its lines: a width is wide enough while the
        // text stays as tall as it is at the full width.
        let full = content.layout(tree, renderer, limits);
        let lines = full.size().height;
        let (mut narrow, mut wide) = (0.0, full.size().width);
        while wide - narrow > PRECISION {
            let width = (narrow + wide) / 2.0;
            if content.layout(tree, renderer, &within(width)).size().height > lines {
                narrow = width;
            } else {
                wide = width;
            }
        }
        // The last layout is the one the content draws with.
        content.layout(tree, renderer, &within(wide))
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
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

impl<'a, Message, Theme, Renderer> From<Balance<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(balance: Balance<'a, Message, Theme, Renderer>) -> Self {
        Element::new(balance)
    }
}
