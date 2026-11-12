//! A transparent wrapper that can force the mouse interaction over its
//! content - the pointer-cursor affordance while the command key is
//! held for cmd+click navigation. (`mouse_area` cannot do this: its
//! interaction only fills in when the content reports none, and a text
//! editor always reports the text cursor.)

use iced::advanced::layout::Limits;
use iced::advanced::widget::{Meta, Operation, Tree};
use iced::advanced::{Layout, Shell, Widget, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Event, Length, Rectangle, Size, Vector};

/// Wraps `content`, forcing `interaction` while the cursor is over it
/// (pass `None` to change nothing).
pub fn pointer<W>(content: W, interaction: Option<mouse::Interaction>) -> Pointer<W> {
    Pointer {
        content,
        interaction,
    }
}

/// See [`pointer`].
#[derive(Debug)]
pub struct Pointer<W> {
    content: W,
    interaction: Option<mouse::Interaction>,
}

impl<W> Meta for Pointer<W> {}

impl<Message, Theme, Renderer, W> Widget<Message, Theme, Renderer> for Pointer<W>
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
        self.content.layout(&mut tree.children[0], renderer, limits);
        tree.size = tree.children[0].size;
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if let Some(interaction) = self.interaction
            && cursor.is_over(layout.bounds())
        {
            return interaction;
        }
        self.content
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
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

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .operate(&mut tree.children[0], layout, viewport, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
            window,
        )
    }
}
