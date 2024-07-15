use std::sync::{Arc, Mutex};

use accesskit::Role;
use masonry::{
    Widget, WidgetId,
    AccessCtx, AccessEvent, BoxConstraints, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx,
    PointerEvent, Size, StatusChange, TextEvent,
};
use smallvec::SmallVec;
use vello::Scene;
use xilem::{Pod, ViewCtx};
use xilem::core::{MessageResult, DynMessage, Mut, View, ViewId};

use crate::game::GameWorld;

pub struct GamePortal {
    game_world: Arc<Mutex<GameWorld>>,
}

impl Widget for GamePortal {
    fn on_pointer_event(&mut self, _: &mut EventCtx<'_>, _: &PointerEvent) {}

    fn on_text_event(&mut self, _: &mut EventCtx<'_>, _: &TextEvent) {}

    fn on_access_event(&mut self, _: &mut EventCtx<'_>, _: &AccessEvent) {}

    fn on_status_change(&mut self, _: &mut LifeCycleCtx<'_>, _: &StatusChange) {}

    fn lifecycle(&mut self, _: &mut LifeCycleCtx<'_>, _: &LifeCycle) {}

    fn layout(&mut self, _: &mut LayoutCtx, bc: &BoxConstraints) -> Size {
        bc.max()
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        let mut game_world = self.game_world.lock().unwrap();
        game_world.render(scene, ctx);
    }

    fn accessibility_role(&self) -> accesskit::Role {
        Role::GenericContainer
    }

    fn accessibility(&mut self, _: &mut AccessCtx<'_>) {}

    fn children_ids(&self) -> SmallVec<[WidgetId; 16]> {
        SmallVec::new()
    }
}

pub struct GameView {
    game_world: Arc<Mutex<GameWorld>>,
}

impl<State, Action> View<State, Action, ViewCtx> for GameView {
    type Element = Pod<GamePortal>;
    type ViewState = ();

    fn build(&self, _ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        let widget = GamePortal {
            game_world: self.game_world.clone(),
        };
        (Pod::new(widget), ())
    }

    fn rebuild<'el>(
        &self,
        _prev: &Self,
        (): &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        element: Mut<'el, Self::Element>,
    ) -> Mut<'el, Self::Element> {
        element
    }

    fn teardown(
        &self,
        (): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        ctx.teardown_leaf(element);
    }

    fn message(
        &self,
        (): &mut Self::ViewState,
        id_path: &[ViewId],
        _message: DynMessage,
        _app_state: &mut State,
    ) -> MessageResult<Action> {
        debug_assert!(
            !id_path.is_empty(),
            "id path should be non-empty in GameView::message"
        );

        // but we haven't set up children yet, so shouldn't be empty either (should just not get here)
        unreachable!("message should not be sent to GameView without child.");
    }
}

impl GameView {
    pub fn new(game_world: Arc<Mutex<GameWorld>>) -> Self {
        Self { game_world }
    }
}
