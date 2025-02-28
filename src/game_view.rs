use std::sync::{Arc, Mutex};

use accesskit::Role;
use masonry::core::{AccessCtx, AccessEvent, BoxConstraints, EventCtx, LayoutCtx, PaintCtx, PointerEvent, RegisterCtx, TextEvent, Widget, WidgetId};
use smallvec::SmallVec;
use vello::Scene;
use xilem::{Pod, ViewCtx};
use xilem::core::{DynMessage, MessageResult, Mut, View, ViewId, ViewMarker};

use crate::game::GameWorld;

pub struct GamePortal {
    game_world: Arc<Mutex<GameWorld>>,
}

impl Widget for GamePortal {
    fn on_pointer_event(&mut self, _: &mut EventCtx<'_>, _: &PointerEvent) {}

    fn on_text_event(&mut self, _: &mut EventCtx<'_>, _: &TextEvent) {}

    fn on_access_event(&mut self, _: &mut EventCtx<'_>, _: &AccessEvent) {}

    fn layout(&mut self, _: &mut LayoutCtx, bc: &BoxConstraints) -> vello::kurbo::Size {
        bc.max()
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        let mut game_world = self.game_world.lock().unwrap();
        game_world.render(scene, ctx);
    }

    fn accessibility_role(&self) -> accesskit::Role {
        Role::GenericContainer
    }

    fn children_ids(&self) -> SmallVec<[WidgetId; 16]> {
        SmallVec::new()
    }

    fn register_children(&mut self, _ctx: &mut RegisterCtx) {}

    fn accessibility(&mut self, _ctx: &mut AccessCtx, _node: &mut accesskit::Node) {}
}

pub struct GameView {
    game_world: Arc<Mutex<GameWorld>>,
}

impl ViewMarker for GameView {}

impl<State, Action> View<State, Action, ViewCtx> for GameView {
    type Element = Pod<GamePortal>;
    type ViewState = ();

    fn build(&self, _ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        let widget = GamePortal {
            game_world: self.game_world.clone(),
        };
        (Pod::new(widget), ())
    }

    fn rebuild(
        &self,
        _prev: &Self,
        _view_state: &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        _element: Mut<'_, Self::Element>,
        ) {
    }

    fn teardown(
        &self,
        _view_state: &mut Self::ViewState,
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
