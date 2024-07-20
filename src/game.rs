use std::{
    collections::HashSet,
    f64::consts::{PI, SQRT_2, TAU},
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
    time::Instant,
};

use masonry::{
    parley::{
        self,
        style::{FontFamily, FontStack, StyleProperty},
    },
    Affine, PaintCtx, Size, Vec2,
};
use vello::Scene;
use winit::{
    event::{DeviceEvent, ElementState, RawKeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::game_shapes::{
    air_pod_scene, air_pod_shape, asteroid_shape, border_shape, flame_scene, ship_shape,
};

const MICROS_PER_SECOND: u64 = 1_000_000;
const TICKS_PER_SECOND: u64 = 30;
// Rounding is fine, this const is authorative, so ~30 ticks/sec
const MICROS_PER_TICK: u64 = MICROS_PER_SECOND / TICKS_PER_SECOND;

const TARGET_FPS: u64 = 60;
const MAX_SHIP_SPEED: f64 = 30.0;

// --- MARK: GameWorld ---

//-------------------------------------------------------------------------
// GameWorld for a simple 2d game.
//-------------------------------------------------------------------------
pub struct GameWorld {
    seed: u64,
    sequence: u32,
    max_radius: f64,
    resources: Resources,
    entity_store: EntityStore,
    spatial_db: SpatialDb,
    input_manager: InputManager,
    exit_ready: bool,
    control_object: Option<EntityId>,
    last_time: Instant,
    last_render: Instant,
    render_ready: bool,
    virtual_time: u128,
    last_tick: u32,
}

impl GameWorld {
    pub fn new(seed: u64, extent: f64) -> Self {
        let entity_store = EntityStore::new();
        let spatial_db = SpatialDb::new(25, extent);
        let resources = Resources::new(extent);

        GameWorld {
            seed,
            sequence: 0,
            max_radius: 0.0,
            resources,
            entity_store,
            spatial_db,
            input_manager: InputManager::new(),
            exit_ready: false,
            control_object: None,
            last_time: Instant::now(),
            last_render: Instant::now(),
            render_ready: true,
            virtual_time: 0,
            last_tick: 0,
        }
    }

    pub fn get_seed(&self) -> u64 {
        self.seed
    }

    pub fn get_sequence(&mut self) -> u32 {
        self.sequence += 1;
        self.sequence
    }

    pub fn is_exit_ready(&self) -> bool {
        self.exit_ready
    }

    pub fn ready_for_redraw(&self) -> bool {
        self.render_ready
    }

    pub fn get_control_object(&self) -> Option<EntityId> {
        self.control_object
    }

    pub fn set_control_object(&mut self, id: EntityId) {
        self.control_object = Some(id);
    }

    pub fn handle_device_event(&mut self, event: &winit::event::DeviceEvent) {
        self.input_manager.input(event);
    }

    pub fn handle_window_key_event(&mut self, event: &winit::event::WindowEvent) {
        if let WindowEvent::KeyboardInput { event, .. } = event {
            // Convert the window key event to a device event
            let raw_key = RawKeyEvent { physical_key: event.physical_key, state: event.state };
            let device_event = DeviceEvent::Key(raw_key);

            self.input_manager.input(&device_event);
        }
    }

    fn add_object(
        &mut self,
        object: GameObject,
        pos_range: Range<Vec2>,
        retry_checks: u32,
        add_anyway: bool,
    ) -> Option<EntityId> {
        let mut object = object;

        let our_rad = object.collision.radius();

        // adjust position range to account for radius
        let mut pos_range = pos_range;
        let range_min = self.spatial_db.get_min() + Vec2::new(our_rad, our_rad);
        let range_max = self.spatial_db.get_max() - Vec2::new(our_rad, our_rad);
        pos_range.start.x = pos_range.start.x.max(range_min.x);
        pos_range.start.y = pos_range.start.y.max(range_min.y);
        pos_range.end.x = pos_range.end.x.min(range_max.x);
        pos_range.end.y = pos_range.end.y.min(range_max.y);

        object.pick_position(self.get_seed(), self.get_sequence(), pos_range.clone());

        if object.collision.radius() > self.max_radius {
            self.max_radius = object.collision.radius();
        }

        for i in 1..=retry_checks {
            let pos = object.transform.translation();
            let mut occupied = false;

            let min_pos = pos - Vec2::new(our_rad, our_rad);
            let max_pos = pos + Vec2::new(our_rad, our_rad);

            self.spatial_db
                .probe_range(min_pos..max_pos, self.max_radius, &mut |other_id| {
                    let other = self.get_entities().get(other_id);
                    let other_pos = other.transform.translation();
                    let dist = (pos - other_pos).length();
                    let min_dist = our_rad + other.collision.radius();
                    if dist < min_dist {
                        occupied = true;
                    }
                });

            if !occupied {
                break;
            }

            if i == retry_checks {
                if !add_anyway {
                    return None;
                }
            }

            object.pick_position(self.get_seed(), self.get_sequence(), pos_range.clone());
        }

        let id = self.get_entities_mut().insert(object);
        let obj = self.entity_store.get_mut(id);
        let pos = obj.transform.translation();
        self.spatial_db.update(id, pos, &mut obj.spatial_db_ref);
        Some(id)
    }

    pub fn get_resources(&self) -> &Resources {
        &self.resources
    }

    pub fn get_entities(&self) -> &EntityStore {
        &self.entity_store
    }

    pub fn get_entities_mut(&mut self) -> &mut EntityStore {
        &mut self.entity_store
    }

    pub fn get_spatial_db(&self) -> &SpatialDb {
        &self.spatial_db
    }

    pub fn add_ship(&mut self, pos_range: Range<Vec2>) -> EntityId {
        let seq = self.get_sequence();
        let ship = GameObject::new_ship(&self.get_resources(), self.get_seed(), seq);

        self.add_object(ship, pos_range, 10, true).unwrap()
    }

    pub fn add_asteroid(
        &mut self,
        pos_range: Range<Vec2>,
        vel_range: Range<f64>,
        ang_vel_range: Range<f64>,
    ) -> Option<EntityId> {
        let seq = self.get_sequence();
        let asteroid = GameObject::new_asteroid(
            &self.get_resources(),
            self.get_seed(),
            seq,
            vel_range,
            ang_vel_range,
        );

        self.add_object(asteroid, pos_range, 10, false)
    }

    pub fn add_air_pod(&mut self, pos_range: Range<Vec2>) -> EntityId {
        let seq = self.get_sequence();
        let air_pod = GameObject::new_air_pod(&self.get_resources(), self.get_seed(), seq);
        self.add_object(air_pod, pos_range, 10, true).unwrap()
    }

    fn update_player_controls(&mut self) {
        let ctrl_id = self.get_control_object();
        if let Some(ctrl_id) = ctrl_id {
            let ctrl_obj = &mut self.entity_store.get_mut(ctrl_id);
            if ctrl_obj.air_suuply.as_ref().map(|air| air.air).unwrap_or(0) == 0 {
                // ship is out of air, no controls
                ctrl_obj.animation = None;
                return;
            }
            let left_down = self.input_manager.is_down(PhysicalKey::Code(KeyCode::ArrowLeft)) || self.input_manager.is_down(PhysicalKey::Code(KeyCode::KeyA));
            let right_down = self.input_manager.is_down(PhysicalKey::Code(KeyCode::ArrowRight)) || self.input_manager.is_down(PhysicalKey::Code(KeyCode::KeyD));
            let thrust_down = self.input_manager.is_down(PhysicalKey::Code(KeyCode::ArrowUp)) || self.input_manager.is_down(PhysicalKey::Code(KeyCode::KeyW));
            match (left_down, right_down) {
                (true, false) => {
                    ctrl_obj.transform.apply_rotation(-0.15);
                }
                (false, true) => {
                    ctrl_obj.transform.apply_rotation(0.15);
                }
                _ => {}
            }
            if thrust_down {
                ctrl_obj.rigid.velocity += 1.0 * ctrl_obj.transform.get_y_vector();
                if ctrl_obj.animation.is_none() {
                    ctrl_obj.animation = Some(Animation {
                        start_time: Instant::now(),
                        animation: flame_scene,
                    });
                }
            } else {
                ctrl_obj.animation = None;
            }
        }
    }

    fn apply_physics(&mut self) {
        for (id, entity) in &mut self.entity_store.iter_mut_entity() {
            let pos = entity.transform.translation();
            let vel = entity.rigid.velocity;
            entity.transform.apply_translation(vel);
            entity
                .transform
                .apply_rotation(entity.rigid.angular_velocity);
            self.spatial_db.update(id, pos, &mut entity.spatial_db_ref);
        }
        for entity in &mut self.entity_store.entities {
            entity.rigid.velocity *= 1.0 - entity.rigid.dampening;
            entity.rigid.angular_velocity *= 1.0 - entity.rigid.angular_dampening;

            if entity.object_type == GameObjectType::Ship {
                let vel = entity.rigid.velocity.length();
                if vel > MAX_SHIP_SPEED {
                    entity.rigid.velocity *= MAX_SHIP_SPEED / vel;
                }
            }
        }
    }

    fn detect_collisions(&mut self, contacts: &mut Vec<Contact>) {
        let max_radius = self.max_radius;

        self.get_spatial_db()
            .find_neighbors(max_radius, &mut |id1, id2| {
                let obj1 = &self.entity_store.entities[id1.0];
                let obj2 = &self.entity_store.entities[id2.0];

                let pos1 = obj1.transform.translation();
                let pos2 = obj2.transform.translation();
                let dist = (pos1 - pos2).length();
                let min_dist = obj1.collision.radius() + obj2.collision.radius();
                if dist < min_dist {
                    // collision
                    let normal = (pos2 - pos1).normalize();
                    let c1 = pos1 + normal * obj1.collision.radius();
                    let c2 = pos2 - normal * obj2.collision.radius();
                    contacts.push(Contact {
                        id1: Some(id1),
                        id2: Some(id2),
                        pos: 0.5 * (c1 + c2),
                        normal1: (pos2 - pos1).normalize(),
                        depth: min_dist - dist,
                    });
                }
            });

        let ul = self.get_spatial_db().get_min();
        let lr = self.get_spatial_db().get_max();
        let ur = Vec2::new(lr.x, ul.y);
        let ll = Vec2::new(ul.x, lr.y);
        self.get_spatial_db()
            .probe_range(ul..ur, max_radius, &mut |id| {
                let obj = self.entity_store.get(id);
                let pos = obj.transform.translation();
                let rad = obj.collision.radius();
                if pos.y - rad < ul.y {
                    // out of bounds
                    contacts.push(Contact {
                        id1: Some(id),
                        id2: None,
                        pos: Vec2::new(pos.x, ul.y),
                        normal1: Vec2::new(0.0, -1.0),
                        depth: ul.y - (pos.y - rad),
                    });
                }
            });

        self.get_spatial_db()
            .probe_range(ll..lr, max_radius, &mut |id| {
                let obj = self.entity_store.get(id);
                let pos = obj.transform.translation();
                let rad = obj.collision.radius();
                if pos.y + rad > ll.y {
                    // out of bounds
                    contacts.push(Contact {
                        id1: Some(id),
                        id2: None,
                        pos: Vec2::new(pos.x, ll.y),
                        normal1: Vec2::new(0.0, 1.0),
                        depth: (pos.y + rad) - ll.y,
                    });
                }
            });
        self.get_spatial_db()
            .probe_range(ul..ll, max_radius, &mut |id| {
                let obj = self.entity_store.get(id);
                let pos = obj.transform.translation();
                let rad = obj.collision.radius();
                if pos.x - rad < ul.x {
                    // out of bounds
                    contacts.push(Contact {
                        id1: Some(id),
                        id2: None,
                        pos: Vec2::new(ul.x, pos.y),
                        normal1: Vec2::new(-1.0, 0.0),
                        depth: ul.x - (pos.x - rad),
                    });
                }
            });
        self.get_spatial_db()
            .probe_range(ur..lr, max_radius, &mut |id| {
                let obj = self.entity_store.get(id);
                let pos = obj.transform.translation();
                let rad = obj.collision.radius();
                if pos.x + rad > ur.x {
                    // out of bounds
                    contacts.push(Contact {
                        id1: Some(id),
                        id2: None,
                        pos: Vec2::new(ur.x, pos.y),
                        normal1: Vec2::new(1.0, 0.0),
                        depth: (pos.x + rad) - ur.x,
                    });
                }
            });
    }

    fn resolve_collisions(&mut self, contacts: &mut Vec<Contact>) {
        let mut dummy_obj = GameObject::new_dummy();

        //
        let mut relocate_air = None;
        let mut ship_loc = None;

        for i in 0..5 {
            for contact in contacts.iter() {
                let id1 = contact.id1.unwrap();

                let (obj1, obj2) = if let Some(id2) = contact.id2 {
                    self.entity_store.get_mut_pair(id1, id2)
                } else {
                    (self.entity_store.get_mut(id1), &mut dummy_obj)
                };

                if (obj1.object_type == GameObjectType::AidPod
                    && obj2.object_type == GameObjectType::Ship)
                    || (obj2.object_type == GameObjectType::AidPod
                        && obj1.object_type == GameObjectType::Ship)
                {
                    // air collection
                    if i == 0 {
                        let (Some(air1), Some(air2)) =
                            (obj1.air_suuply.as_mut(), obj2.air_suuply.as_mut())
                        else {
                            continue;
                        };
                        if relocate_air.is_some() {
                            // possible to have same collision twice, so make sure to only do this once
                            continue;
                        }
                        if obj1.object_type == GameObjectType::Ship {
                            air1.air += air2.air;
                            if let Some(score) = obj1.score.as_mut() {
                                score.0 += air2.air + 1000;
                            }

                            // save some data for finding next air pod location
                            relocate_air = contact.id2;
                            ship_loc = Some(obj1.transform.translation());
                            println!(
                                "Ship collects {} air, raising total to {}",
                                air2.air, air1.air
                            );
                        } else {
                            air2.air += air1.air;
                            if let Some(score) = obj2.score.as_mut() {
                                score.0 += air1.air + 1000;
                            }

                            // save some data for finding next air pod location
                            relocate_air = contact.id1;
                            ship_loc = Some(obj2.transform.translation());
                            println!(
                                "Ship collects {} air, raising total to {}",
                                air1.air, air2.air
                            );
                        }
                    }
                    continue;
                }

                // get relative velocity of contact points on obj1 and obj2
                let offset1 = contact.pos - obj1.transform.translation();
                let offset2 = contact.pos - obj2.transform.translation();
                let v1 = obj1.rigid.get_world_offset_vel(&offset1);
                let v2: Vec2 = obj2.rigid.get_world_offset_vel(&offset2);
                let delta_vel = v2 - v1;
                let contact_vel = delta_vel.dot(contact.normal1);
                let tangent_vel = delta_vel - contact_vel * contact.normal1;

                let inv_mass1 = obj1.rigid.inv_mass;
                let inv_mass2 = obj2.rigid.inv_mass;
                let inv_inertia1 = obj1.rigid.inv_ang_inertia_sqrt;
                let inv_inertia2 = obj2.rigid.inv_ang_inertia_sqrt;

                let cross1 =
                    (offset1.x * contact.normal1.y - offset1.y * contact.normal1.x) * inv_inertia1;
                let cross2 =
                    (-offset2.x * contact.normal1.y + offset2.y * contact.normal1.x) * inv_inertia2;
                let inv_mass_inertia = inv_mass1 + inv_mass2 + cross1 * cross1 + cross2 * cross2;

                if contact_vel >= 0.0 {
                    // moving apart...
                    continue;
                }

                if i == 0 && tangent_vel.length_squared() > 1e-4 {
                    // apply a frictional force to asteroids. Since everything is a circle, this is the only
                    // way we get angular velocity. Ship and air pod objects are not affected.

                    let friction_coeff = 0.25;
                    let tangent_impulse = friction_coeff * tangent_vel / inv_mass_inertia;

                    if obj1.object_type == GameObjectType::Asteroid {
                        obj1.rigid.apply_impulse(tangent_impulse, offset1);
                    }
                    if obj2.object_type == GameObjectType::Asteroid {
                        obj2.rigid.apply_impulse(-tangent_impulse, offset2);
                    }
                }

                // Restitution is min of restitutions.
                let restitution = obj1.rigid.restitution.min(obj2.rigid.restitution);

                let mag = (1.0 + restitution) * contact_vel / inv_mass_inertia;

                let impulse = contact.normal1 * mag;
                obj1.rigid.apply_impulse(impulse, offset1);
                if obj2.object_type != GameObjectType::Dummy {
                    obj2.rigid.apply_impulse(-impulse, offset2);
                }
            }
        }

        // one more pass to apply anti-penetration force
        for contact in contacts.iter() {
            let id1 = contact.id1.unwrap();

            let (obj1, obj2) = if let Some(id2) = contact.id2 {
                self.entity_store.get_mut_pair(id1, id2)
            } else {
                (self.entity_store.get_mut(id1), &mut dummy_obj)
            };

            if (obj1.object_type == GameObjectType::AidPod
                && obj2.object_type == GameObjectType::Ship)
                || (obj2.object_type == GameObjectType::AidPod
                    && obj1.object_type == GameObjectType::Ship)
            {
                continue;
            }

            // apply position correction, moving in proportion to mass
            let percent = 0.5;
            let inv_mass1 = obj1.rigid.inv_mass;
            let inv_mass2 = obj2.rigid.inv_mass;
            let correction =
                contact.normal1 * percent * contact.depth.max(0.0) / (inv_mass1 + inv_mass2);
            obj1.transform.apply_translation(-correction * inv_mass1);
            obj2.transform.apply_translation(correction * inv_mass2);
        }

        // slip this in here but really this is nothing to do with resolving collisions,
        // this is responding to special collision between ship and air pod
        if let Some(air_id) = relocate_air {
            let seq = self.get_sequence();
            let air = self.entity_store.get_mut(air_id);
            air.pick_position(
                self.seed,
                seq,
                self.spatial_db.get_min()..self.spatial_db.get_max(),
            );

            // use distance of pod from ship and max speed ship can travel to determine air supply
            let dist = (air.transform.translation() - ship_loc.unwrap()).length();
            let time = dist / MAX_SHIP_SPEED; // speed is measured in units/tick (TODO: convert to time)
            let mult = 4.0;
            air.air_suuply = Some(AirSupply {
                air: (mult * time) as u64,
            });
        }
    }

    fn check_air(&mut self) {
        for obj in &mut self.entity_store.entities {
            if let Some(air) = obj.air_suuply.as_mut() {
                air.air = air.air.saturating_sub(1);
            }
        }
    }
    fn flip_transforms(&mut self) {
        for entity in &mut self.entity_store.entities {
            entity.prev_transform = entity.transform.clone();
        }
    }

    pub fn interpolate_transforms(&mut self) {
        let interp = self.get_interp();
        for entity in &mut self.entity_store.entities {
            entity.render_transform.translation = entity
                .prev_transform
                .translation
                .lerp(entity.transform.translation, interp);
            let delta_rot = entity.transform.rotation - entity.prev_transform.rotation;
            let delta_rot = if delta_rot > PI {
                delta_rot - TAU
            } else if delta_rot < -PI {
                delta_rot + TAU
            } else {
                delta_rot
            };
            entity.render_transform.rotation = entity.prev_transform.rotation + interp * delta_rot;
        }
    }

    fn update_time(&mut self) -> u32 {
        let now = Instant::now();
        let elapsed = now - self.last_time;
        self.last_time = now;

        let elapsed = elapsed.as_micros();

        self.virtual_time += elapsed;
        let tick = (self.virtual_time / MICROS_PER_TICK as u128) as u32;

        let num_tick = tick - self.last_tick;
        self.last_tick = tick;

        // This is a bit awkward doing this here (and storing as bool) but we don't pass mutable self to render
        // so this is most convenient
        self.render_ready =
            self.last_render.elapsed().as_micros() as u64 > MICROS_PER_SECOND / TARGET_FPS;
        // HACK: turn off frame rate cap for now since it seems to cause backoff stragegy for some event loops.
        self.render_ready = true;
        if self.render_ready {
            self.last_render = now;
        }

        num_tick
    }

    pub fn get_interp(&self) -> f64 {
        let interp = self.virtual_time % MICROS_PER_TICK as u128;
        let interp = interp as f64 / MICROS_PER_TICK as f64;
        interp
    }

    pub fn update(&mut self) {
        let num_tick = self.update_time();

        // Set exit on make or break event just for code coverage
        let esc = PhysicalKey::Code(KeyCode::Escape);
        if self.input_manager.is_break(esc) || self.input_manager.is_make(esc) {
            self.exit_ready = true;
        }

        for _ in 0..num_tick {
            self.flip_transforms();
            self.update_player_controls();
            self.apply_physics();

            let mut contacts = Vec::new();
            self.detect_collisions(&mut contacts);
            self.resolve_collisions(&mut contacts);

            self.check_air();

            // this goes here, so if more than one tick processed the make/break
            // events won't be processed more than once
            self.input_manager.clear_events();
        }
    }

    fn render_game_state(&self, scene: &mut Scene, ctx: &mut PaintCtx, size: Size) {
        let min_dim = size.width.min(size.height);
        let margin = 0.05 * min_dim;

        let Some(player) = self
            .get_control_object()
            .map(|id| self.get_entities().get(id))
        else {
            // no player no game state
            return;
        };

        let score = format!("Score: {}", player.score.map(|score| score.0).unwrap_or(0));
        let air = format!(
            "Air: {:.1} seconds",
            player.air_suuply.as_ref().map_or(0, |air| air.air) as f32 / TICKS_PER_SECOND as f32
        );
        let txt = format!("{}\n{}", score, air);

        let fill_color = xilem::Color::rgb8(0xff, 0xff, 0xff);

        // To render text, we first create a LayoutBuilder and set the text properties.
        let mut lcx = masonry::parley::LayoutContext::new();
        let mut text_layout_builder = lcx.ranged_builder(ctx.text_contexts().0, &txt, 1.0);

        text_layout_builder.push_default(&StyleProperty::FontStack(FontStack::Single(
            FontFamily::Generic(parley::style::GenericFamily::Serif),
        )));
        text_layout_builder.push_default(&StyleProperty::FontSize(24.0));
        text_layout_builder.push_default(&StyleProperty::Brush(
            vello::peniko::Brush::Solid(fill_color).into(),
        ));

        let mut text_layout = text_layout_builder.build();
        text_layout.break_all_lines(None, xilem::TextAlignment::Start);

        let mut scratch_scene = Scene::new();
        // We can pass a transform matrix to rotate the text we render
        masonry::text_helpers::render_text(
            scene,
            &mut scratch_scene,
            Affine::translate(Vec2::new(margin, margin)),
            &text_layout,
        );

        if player.air_suuply.as_ref().map(|air| air.air).unwrap_or(0) == 0 {
            // Game Over
            let txt = "    GAME OVER\nYou are out of air!";
            let fill_color = xilem::Color::rgb8(0xff, 0x00, 0x00);

            let mut lcx = masonry::parley::LayoutContext::new();
            let mut text_layout_builder = lcx.ranged_builder(ctx.text_contexts().0, &txt, 1.0);

            text_layout_builder.push_default(&StyleProperty::FontStack(FontStack::Single(
                FontFamily::Generic(parley::style::GenericFamily::Serif),
            )));
            text_layout_builder.push_default(&StyleProperty::FontSize(48.0));
            text_layout_builder.push_default(&StyleProperty::Brush(
                vello::peniko::Brush::Solid(fill_color).into(),
            ));

            let mut text_layout = text_layout_builder.build();
            text_layout.break_all_lines(None, xilem::TextAlignment::Middle);
            let w = text_layout.width();
            let h = text_layout.height();

            let mut scratch_scene = Scene::new();
            // We can pass a transform matrix to rotate the text we render
            masonry::text_helpers::render_text(
                scene,
                &mut scratch_scene,
                Affine::translate(Vec2::new(
                    0.5 * (size.width - w as f64),
                    0.5 * (size.height - h as f64),
                )),
                &text_layout,
            );
        }
    }

    fn render_mini_map(&self, scene: &mut Scene, size: Size, cam_pos: Vec2) {
        let min_dim = size.width.min(size.height);
        let map_size = 0.25 * min_dim;
        let map_radius = 0.5 * map_size;
        let margin = 0.05 * min_dim;

        let render_radius = 4000.0;
        let map_scale = map_size / render_radius;

        // render mini-map in top right corner, with margin
        let map_center = masonry::Point::new(size.width - map_radius - margin, map_radius + margin);
        let world_to_map = Affine::translate(-cam_pos)
            .then_scale(map_scale)
            .then_translate(map_center.to_vec2());

        scene.push_layer(
            vello::peniko::BlendMode::default(),
            1.0,
            Affine::IDENTITY,
            &vello::kurbo::Circle::new(map_center, map_radius),
        );

        scene.fill(
            vello::peniko::Fill::NonZero,
            Affine::IDENTITY,
            xilem::Color::rgb8(0, 0, 0),
            None,
            &vello::kurbo::Circle::new(map_center, map_radius),
        );

        // compute oscillation for air animation, TODO: oscillate in sync with animation, make rate a function of air left
        let t = self.virtual_time as f64 / MICROS_PER_SECOND as f64;
        let rate = 4.0;
        let oscillation = ((t % (1.0 / rate)) - 0.5 / rate).abs() * 2.0 * rate;

        for entity in &self.entity_store.entities {
            let color = match entity.object_type {
                GameObjectType::Ship => xilem::Color::rgb8(0xff, 0xff, 0xff),
                GameObjectType::Asteroid => xilem::Color::rgb8(0x7f, 0x7f, 0x7f),
                GameObjectType::AidPod => xilem::Color::rgb8(0x0, 0xb4, 0xd8),
                GameObjectType::Dummy => unreachable!("Dummy object in render"),
            };
            let radius_scale = match entity.object_type {
                GameObjectType::Ship => 2.0,
                GameObjectType::Asteroid => 1.0,
                GameObjectType::AidPod => 2.0 * (0.1 + 0.9 * oscillation),
                GameObjectType::Dummy => unreachable!("Dummy object in render"),
            };
            let radius = radius_scale * entity.collision.radius();

            let pos = world_to_map * entity.render_transform.translation().to_point();

            let dist = pos.distance(map_center);
            if dist - map_scale * radius > map_radius
                && entity.object_type != GameObjectType::AidPod
            {
                // object is off screen, don't render
                continue;
            }

            let pos = if dist - map_scale * radius > map_radius {
                // this is only for air object
                let dir = (pos - map_center).normalize();
                map_center + map_radius * dir
            } else {
                pos
            };

            if let Some(shape) = entity.shape.as_ref() {
                // render asteroid or ship
                let transform = Affine::rotate(entity.transform.rotation)
                    .then_scale(map_scale * radius_scale)
                    .then_translate(pos.to_vec2());
                scene.append(shape.scene(), Some(transform));
            } else {
                // render flashing blue dot for air
                scene.fill(
                    vello::peniko::Fill::NonZero,
                    Affine::translate(pos.to_vec2()),
                    color,
                    None,
                    &vello::kurbo::Circle::new((0.0, 0.0), map_scale * radius),
                );
            }
        }

        scene.append(
            self.get_resources().border_shape.scene(),
            Some(world_to_map),
        );

        scene.pop_layer();

        scene.stroke(
            &vello::kurbo::Stroke::new(4.0),
            Affine::IDENTITY,
            xilem::Color::rgb8(0xff, 0xff, 0xff),
            None,
            &vello::kurbo::Circle::new(map_center, 0.5 * map_size),
        );
    }

    pub fn render(&mut self, scene: &mut Scene, ctx: &mut PaintCtx) {
        let size = ctx.size();
        let ctrl_id = self.control_object;
        let cam_pos = if let Some(ctrl_id) = ctrl_id {
            let ctrl = &self.entity_store.entities[ctrl_id.0];
            ctrl.render_transform.translation()
        } else {
            Vec2::new(0.0, 0.0)
        };

        for entity in &self.entity_store.entities {
            if entity.object_type == GameObjectType::AidPod {
                // if air pod is off screen, render blip at edge of screen
                let rad = entity.collision.radius();
                let half_size = 0.5 * size.to_vec2();
                let pos = entity.render_transform.translation() - cam_pos;
                if pos.x + rad < -half_size.x
                    || pos.x - rad > half_size.x
                    || pos.y + rad < -half_size.y
                    || pos.y - rad > half_size.y
                {
                    let clip_end = |p0, p1, c0, c1, c_clip| -> Vec2 {
                        let t = (c_clip - c0) / (c1 - c0);
                        if t < 0.0 {
                            p1
                        } else if t > 1.0 {
                            p1
                        } else {
                            p0 + t * (p1 - p0)
                        }
                    };

                    let p0 = Vec2::new(0.0, 0.0);
                    let pos = clip_end(p0, pos, 0.0, pos.x, -half_size.x);
                    let pos = clip_end(p0, pos, 0.0, pos.x, half_size.x);
                    let pos = clip_end(p0, pos, 0.0, pos.y, -half_size.y);
                    let pos = clip_end(p0, pos, 0.0, pos.y, half_size.y);

                    // compute oscillation for air animation on edge of screen. This is copy-pasted from minimap
                    let t = self.virtual_time as f64 / MICROS_PER_SECOND as f64;
                    let rate = 4.0;
                    let oscillation = ((t % (1.0 / rate)) - 0.5 / rate).abs() * 2.0 * rate;

                    scene.fill(
                        vello::peniko::Fill::NonZero,
                        Affine::translate(pos + half_size),
                        xilem::Color::rgb8(0x0, 0xd4, 0xf8),
                        None,
                        &vello::kurbo::Circle::new((0.0, 0.0), 16.0 + oscillation * 48.0),
                    );
                    continue;
                }
            }
            let transform = Affine::rotate(entity.render_transform.rotation()).then_translate(
                entity.render_transform.translation() - cam_pos + 0.5 * size.to_vec2(),
            );
            if let Some(animation) = &entity.animation {
                let elapsed = animation.start_time.elapsed().as_secs_f64();
                let animation = (animation.animation)(elapsed);

                scene.append(&animation, Some(transform));
            }

            if let Some(shape) = &entity.shape {
                scene.append(shape.scene(), Some(transform));
            }
        }
        let border_transform = Affine::translate(-cam_pos + 0.5 * size.to_vec2());
        scene.append(
            self.get_resources().border_shape.scene(),
            Some(border_transform),
        );

        self.render_mini_map(scene, size, cam_pos);
        self.render_game_state(scene, ctx, size);
    }
}

// --- MARK: GameObject ---

//-------------------------------------------------------------------------
// GameObject: every game object has all the components, but some are optional.
// In a larger game you would use an ecs sysem like hecs.
//----------------------------------------------------------------------

pub struct GameObject {
    pub transform: Transform,
    pub prev_transform: Transform,
    pub render_transform: Transform,
    pub spatial_db_ref: SpatialDbRef,
    pub collision: Collision,
    pub rigid: Rigid,
    pub shape: Option<Shape>,
    pub animation: Option<Animation>,
    pub air_suuply: Option<AirSupply>,
    pub score: Option<Score>,
    pub object_type: GameObjectType,
}

impl GameObject {
    fn new_ship(resources: &Resources, _seed: u64, _seq: u32) -> Self {
        let shape = resources.ship_shape.clone();
        let collision = Collision::new(shape.radius());
        let spatial_db_ref = SpatialDbRef {
            spatial_id: SpatialId::new(),
        };
        let rigid = Rigid::new(shape.radius(), 1.0, 0.0, 0.01, 1.0, 0.3);

        GameObject {
            transform: Transform::new(Vec2::ZERO, PI),
            prev_transform: Transform::new(Vec2::ZERO, PI),
            render_transform: Transform::new(Vec2::ZERO, PI),
            spatial_db_ref,
            collision,
            rigid,
            shape: Some(shape),
            animation: None,
            air_suuply: Some(AirSupply {
                air: TICKS_PER_SECOND * 60,
            }),
            score: Some(Score(0)),
            object_type: GameObjectType::Ship,
        }
    }

    fn new_air_pod(_resources: &Resources, _seed: u64, _seq: u32) -> Self {
        // get air pod shape at first frame to figure out radius
        let shape = air_pod_shape(0.0);

        let collision = Collision::new(shape.radius());
        let spatial_db_ref = SpatialDbRef {
            spatial_id: SpatialId::new(),
        };
        let rigid = Rigid::new(shape.radius(), 1.0, 0.0, 0.01, 0.99, 0.3);

        GameObject {
            transform: Transform::identity(),
            prev_transform: Transform::identity(),
            render_transform: Transform::identity(),
            spatial_db_ref,
            collision,
            rigid,
            shape: None,
            animation: Some(Animation {
                start_time: Instant::now(),
                animation: air_pod_scene,
            }),
            air_suuply: Some(AirSupply {
                air: TICKS_PER_SECOND * 15,
            }),
            score: None,
            object_type: GameObjectType::AidPod,
        }
    }

    fn new_asteroid(
        resources: &Resources,
        seed: u64,
        seq: u32,
        vel_range: Range<f64>,
        ang_vel_range: Range<f64>,
    ) -> Self {
        let vel = vel_range.hash_rand(seed, (seq, "vel"));
        let vel_angle = (0.0..TAU).hash_rand(seed, (seq, "vel_angle"));
        let vel = Vec2::new(vel * vel_angle.cos(), vel * vel_angle.sin());
        let ang_vel = ang_vel_range.hash_rand(seed, (seq, "ang_vel"));

        let asteroid_num = (0..6).hash_rand(seed, (seq, "asteroid_num"));
        let shape = match asteroid_num {
            0 => resources.small_asteroid1.clone(),
            1 => resources.small_asteroid2.clone(),
            2 => resources.medium_asteroid1.clone(),
            3 => resources.medium_asteroid2.clone(),
            4 => resources.large_asteroid1.clone(),
            5 => resources.large_asteroid2.clone(),
            _ => panic!("Invalid asteroid_num"),
        };

        let collision = Collision::new(shape.radius());
        let spatial_db_ref = SpatialDbRef {
            spatial_id: SpatialId::new(),
        };
        // Note: resitution is 1.01 in order to add a little entergy to the system when asteroids collide, picking up intensity
        let mut rigid = Rigid::new(shape.radius(), 1.5, 1.0, 0.0, 0.0, 1.01);
        rigid.velocity = vel;
        rigid.angular_velocity = ang_vel;

        GameObject {
            transform: Transform::identity(),
            prev_transform: Transform::identity(),
            render_transform: Transform::identity(),
            spatial_db_ref,
            collision,
            rigid,
            shape: Some(shape),
            animation: None,
            air_suuply: None,
            score: None,
            object_type: GameObjectType::Asteroid,
        }
    }

    fn new_dummy() -> Self {
        GameObject {
            transform: Transform::identity(),
            prev_transform: Transform::identity(),
            render_transform: Transform::identity(),
            spatial_db_ref: SpatialDbRef {
                spatial_id: SpatialId::new(),
            },
            collision: Collision::new(0.0),
            rigid: Rigid::new(0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
            shape: None,
            animation: None,
            air_suuply: None,
            score: None,
            object_type: GameObjectType::Dummy,
        }
    }

    fn pick_position(&mut self, seed: u64, seq: u32, pos_range: Range<Vec2>) {
        let pos = pos_range.hash_rand(seed, seq);
        self.transform.translation = pos;
        self.prev_transform.translation = pos;
    }
}

#[derive(PartialEq)]
pub enum GameObjectType {
    Ship,
    Asteroid,
    AidPod,
    Dummy,
}

#[derive(Clone, Copy, Debug)]
pub struct Score(pub u64);

// --- MARK: EntityStore ---

//-------------------------------------------------------------------------
// EntityStore for GameObject that includes all our components.
// EntityStore and GameObject could be replaced with a  generic entity
// component system like HECS.
//-------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct EntityId(usize);

pub struct EntityStore {
    entities: Vec<GameObject>,
}

impl EntityStore {
    pub fn new() -> Self {
        EntityStore {
            entities: Vec::new(),
        }
    }

    pub fn get(&self, id: EntityId) -> &GameObject {
        &self.entities[id.0]
    }

    pub fn get_mut(&mut self, id: EntityId) -> &mut GameObject {
        &mut self.entities[id.0]
    }

    pub fn get_mut_pair(
        &mut self,
        id1: EntityId,
        id2: EntityId,
    ) -> (&mut GameObject, &mut GameObject) {
        if id1.0 < id2.0 {
            let (split1, split2) = self.entities.split_at_mut(id2.0);
            (&mut split1[id1.0], &mut split2[0])
        } else if id1.0 > id2.0 {
            let (split1, split2) = self.entities.split_at_mut(id1.0);
            (&mut split2[0], &mut split1[id2.0])
        } else {
            panic!("Cannot get pair of same id");
        }
    }

    pub fn insert(&mut self, object: GameObject) -> EntityId {
        let id = EntityId(self.entities.len());
        self.entities.push(object);
        id
    }

    // pub fn iter_entity(&self) -> impl Iterator<Item = (EntityId, &GameObject)> {
    //     self.entities.iter().enumerate().map(|(idx, obj)| (EntityId(idx), obj))
    // }

    pub fn iter_mut_entity(&mut self) -> impl Iterator<Item = (EntityId, &mut GameObject)> {
        self.entities
            .iter_mut()
            .enumerate()
            .map(|(idx, obj)| (EntityId(idx), obj))
    }
}

// --- MARK: Shape ---

//-------------------------------------------------------------------------
// Shape component for rendering a static shape
//-------------------------------------------------------------------------
#[derive(Clone)]
pub struct Shape {
    scene: Arc<Scene>,
    radius: f64,
}

impl Shape {
    pub fn new(scene: Arc<Scene>, radius: f64) -> Self {
        Shape { scene, radius }
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn radius(&self) -> f64 {
        self.radius
    }
}

// --- MARK: Animation ---

//-------------------------------------------------------------------------
// Animation component for rendering an animated shape
//-------------------------------------------------------------------------
pub struct Animation {
    pub start_time: Instant,
    pub animation: fn(f64) -> Scene,
}

//-------------------------------------------------------------------------
// Game component for tracking air supply. Air pod and ship have this
// component. Every tick one unit of air is lost. Ship picking up air
// pod adds remaining air in pod to ship's supply.
//-------------------------------------------------------------------------
pub struct AirSupply {
    pub air: u64,
}

// --- MARK: Collision ---

//-------------------------------------------------------------------------
// Simple collision component -- everything is a circle.
//-------------------------------------------------------------------------
pub struct Collision {
    // we're all spheres
    radius: f64,
}

impl Collision {
    pub fn new(radius: f64) -> Self {
        Collision { radius }
    }

    pub fn radius(&self) -> f64 {
        self.radius
    }
}

#[derive(Debug)]
pub struct Contact {
    id1: Option<EntityId>,
    id2: Option<EntityId>,
    pos: Vec2,
    normal1: Vec2,
    // normal2 is -normal1
    depth: f64,
}

// --- MARK: Transform ---

//-------------------------------------------------------------------------
// Transform component
//-------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Transform {
    translation: Vec2,
    rotation: f64,
}

impl Transform {
    pub fn new(translation: Vec2, rotation: f64) -> Self {
        Transform {
            translation,
            rotation,
        }
    }

    pub fn identity() -> Self {
        Transform {
            translation: Vec2::new(0.0, 0.0),
            rotation: 0.0,
        }
    }

    pub fn translation(&self) -> Vec2 {
        self.translation
    }

    pub fn rotation(&self) -> f64 {
        self.rotation
    }

    pub fn apply_rotation(&mut self, rotation: f64) {
        self.rotation = (self.rotation + rotation) % TAU;
    }

    pub fn apply_translation(&mut self, translation: Vec2) {
        self.translation += translation;
    }

    // pub fn get_x_vector(&self) -> Vec2 {
    //     Vec2::new(self.rotation.cos(), self.rotation.sin())
    // }

    pub fn get_y_vector(&self) -> Vec2 {
        Vec2::new(-self.rotation.sin(), self.rotation.cos())
    }
}

// --- MARK: Ridig body ---

//-------------------------------------------------------------------------
// Rigid body component
//-------------------------------------------------------------------------

pub struct Rigid {
    velocity: Vec2,
    angular_velocity: f64,
    dampening: f64,
    angular_dampening: f64,
    restitution: f64,
    inv_mass: f64,
    // simplified inertia since we're all circles here
    inv_ang_inertia_sqrt: f64,
}

impl Rigid {
    pub fn new(
        radius: f64,
        density: f64,
        ang_density: f64,
        dampening: f64,
        ang_dampening: f64,
        restitution: f64,
    ) -> Self {
        let inv_mass = if density > 0.01 {
            1.0 / (density * PI * radius * radius)
        } else {
            0.0
        };
        let inv_ang_inertia_sqrt = if ang_density > 0.01 {
            SQRT_2 / ((ang_density * PI).sqrt() * radius * radius)
        } else {
            0.0
        };
        Self {
            velocity: Vec2::new(0.0, 0.0),
            angular_velocity: 0.0,
            dampening,
            angular_dampening: ang_dampening,
            restitution,
            inv_mass,
            inv_ang_inertia_sqrt,
        }
    }

    #[inline]
    pub fn get_world_offset_vel(&self, offset: &Vec2) -> Vec2 {
        self.velocity
            + Vec2::new(
                -self.angular_velocity * offset.y,
                self.angular_velocity * offset.x,
            )
    }

    pub fn apply_impulse(&mut self, impulse: Vec2, offset: Vec2) {
        self.velocity += impulse * self.inv_mass;
        self.angular_velocity += (offset.x * impulse.y - offset.y * impulse.x)
            * self.inv_ang_inertia_sqrt
            * self.inv_ang_inertia_sqrt;
    }
}

//-------------------------------------------------------------------------
// Component for tracking objects in spatial db
//-------------------------------------------------------------------------

pub struct SpatialDbRef {
    spatial_id: SpatialId,
}

// --- MARK: SpatialDb ---

//-------------------------------------------------------------------------
// Simple grid based spatial database. Could be replaced with a more
// sophisticated spatial database like an AABB tree (e.g., parry2d).
// But this provides a very efficient broad phase collision method.
//-------------------------------------------------------------------------

pub struct SpatialDb {
    dim: u32,
    node_size: f64,
    min: Vec2,
    max: Vec2,
    nodes: Vec<SpatialDbNode>,
}

impl SpatialDb {
    pub fn new(dim: u32, extent: f64) -> Self {
        let node_size = 2.0 * extent / dim as f64;
        let min = Vec2::new(-extent, -extent);
        let max = Vec2::new(extent, extent);

        let mut nodes = Vec::new();
        nodes.resize_with(dim as usize * dim as usize, Default::default);

        SpatialDb {
            dim,
            node_size,
            min,
            max,
            nodes,
        }
    }

    pub fn get_min(&self) -> Vec2 {
        self.min
    }

    pub fn get_max(&self) -> Vec2 {
        self.max
    }

    fn get_spatial_id(&self, pos: Vec2) -> SpatialId {
        // clamp x and y to valid range (border nodes will have infinte range)

        let x = if pos.x <= self.min.x {
            0
        } else if pos.x >= self.max.x {
            self.dim - 1
        } else {
            ((pos.x - self.min.x) / self.node_size) as u32
        };

        let y = if pos.y <= self.min.y {
            0
        } else if pos.y >= self.max.y {
            self.dim - 1
        } else {
            ((pos.y - self.min.y) / self.node_size) as u32
        };

        SpatialId(x + y * self.dim)
    }

    pub fn probe_range(
        &self,
        pos_range: Range<Vec2>,
        max_radius: f64,
        callback: &mut impl FnMut(EntityId),
    ) {
        let minx = ((pos_range.start.x - max_radius - self.min.x).max(0.0) / self.node_size) as u32;
        let maxx = (((pos_range.end.x + max_radius - self.min.x) / self.node_size) as u32)
            .min(self.dim - 1);
        let miny = ((pos_range.start.y - max_radius - self.min.y).max(0.0) / self.node_size) as u32;
        let maxy = (((pos_range.end.y + max_radius - self.min.y) / self.node_size) as u32)
            .min(self.dim - 1);

        for y in miny..=maxy {
            for x in minx..=maxx {
                let idx = (x + y * self.dim) as usize;
                let node = &self.nodes[idx];
                for obj in &node.objects {
                    callback(*obj);
                }
            }
        }
    }

    pub fn update(&mut self, entity_id: EntityId, pos: Vec2, spatial_ref: &mut SpatialDbRef) {
        let new_spatial_id = self.get_spatial_id(pos);

        if new_spatial_id.0 == spatial_ref.spatial_id.0 {
            return;
        }

        // moving ref to new node so removed from old node
        self.remove(entity_id, spatial_ref);

        let node = &mut self.nodes[new_spatial_id.0 as usize];
        node.objects.push(entity_id);
        spatial_ref.spatial_id = new_spatial_id;
    }

    pub fn remove(&mut self, entity_id: EntityId, spatial_ref: &mut SpatialDbRef) {
        if !spatial_ref.spatial_id.is_valid() {
            return;
        }

        let node = &mut self.nodes[spatial_ref.spatial_id.0 as usize];
        for (idx, obj) in node.objects.iter().enumerate() {
            if obj.0 == entity_id.0 {
                node.objects.swap_remove(idx);
                break;
            }
        }

        spatial_ref.spatial_id = SpatialId::new();
    }

    pub fn find_neighbors(&self, max_radius: f64, callback: &mut impl FnMut(EntityId, EntityId)) {
        let num_check_nodes = (2.0 * max_radius / self.node_size) as u32 + 1;

        for y in 0..self.dim {
            for x in 0..self.dim {
                let idx = (x + y * self.dim) as usize;
                let node = &self.nodes[idx];
                if node.objects.is_empty() {
                    continue;
                }

                for y2 in
                    y.saturating_sub(num_check_nodes)..=(y + num_check_nodes).min(self.dim - 1)
                {
                    // don't need to check left side of node because left side will have already checked against us
                    // or will when y2 loop gets there
                    for x2 in x..=(x + num_check_nodes).min(self.dim - 1) {
                        let other_idx = (x2 + y2 * self.dim) as usize;
                        let other_node = &self.nodes[other_idx];
                        if other_node.objects.is_empty() {
                            continue;
                        }

                        // compare our node to node within max radius (only need to check + direction)
                        self.broad_phase_node_node(node, other_node, other_idx == idx, callback);
                    }
                }
            }
        }
    }

    #[inline]
    fn broad_phase_node_node(
        &self,
        node: &SpatialDbNode,
        other_node: &SpatialDbNode,
        same_node: bool,
        callback: &mut impl FnMut(EntityId, EntityId),
    ) {
        for obj in &node.objects {
            for other_obj in &other_node.objects {
                if same_node && obj.0 >= other_obj.0 {
                    // only need to check one time (and no times when same object)
                    continue;
                }
                callback(*obj, *other_obj);
            }
        }
    }
}

struct SpatialId(u32);

impl SpatialId {
    pub fn new() -> Self {
        SpatialId(u32::MAX)
    }

    pub fn is_valid(&self) -> bool {
        self.0 != u32::MAX
    }
}

#[derive(Default)]
struct SpatialDbNode {
    objects: smallvec::SmallVec<[EntityId; 16]>,
}

// --- MARK: Resources ---

//-------------------------------------------------------------------------
// Resources for the game. Fixed set of resources, but could be replaced
// with a more traditional resource manager with loading and unloading.
//-------------------------------------------------------------------------

pub struct Resources {
    pub ship_shape: Shape,
    pub small_asteroid1: Shape,
    pub small_asteroid2: Shape,
    pub medium_asteroid1: Shape,
    pub medium_asteroid2: Shape,
    pub large_asteroid1: Shape,
    pub large_asteroid2: Shape,
    pub border_shape: Shape,
}

impl Resources {
    pub fn new(extent: f64) -> Self {
        Resources {
            ship_shape: ship_shape(),
            small_asteroid1: asteroid_shape(0, 30.0),
            small_asteroid2: asteroid_shape(1, 30.0),
            medium_asteroid1: asteroid_shape(2, 100.0),
            medium_asteroid2: asteroid_shape(3, 100.0),
            large_asteroid1: asteroid_shape(4, 150.0),
            large_asteroid2: asteroid_shape(5, 150.0),
            border_shape: border_shape(extent),
        }
    }
}

// --- MARK: InputManager ---

//-------------------------------------------------------------------------
// InputManager for handling input events.
//-------------------------------------------------------------------------

pub struct InputManager {
    make_events: Vec<PhysicalKey>,
    break_events: Vec<PhysicalKey>,
    key_down: HashSet<PhysicalKey>,
}

impl InputManager {
    pub fn new() -> Self {
        Self {
            make_events: Vec::default(),
            break_events: Vec::default(),
            key_down: HashSet::default(),
        }
    }

    pub fn input(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::Key(key) => {
                if key.state == ElementState::Pressed {
                    self.make_events.push(key.physical_key.clone());
                    self.key_down.insert(key.physical_key.clone());
                } else {
                    self.break_events.push(key.physical_key.clone());
                    self.key_down.remove(&key.physical_key);
                }
            }
            _ => {}
        }
        // We don't really care if key is consumed or not for this simple input manager
        false
    }

    pub fn is_down(&self, key: PhysicalKey) -> bool {
        self.key_down.contains(&key)
    }

    pub fn is_make(&self, key: PhysicalKey) -> bool {
        for k in self.make_events.iter() {
            if *k == key {
                return true;
            }
        }
        return false;
    }

    pub fn is_break(&self, key: PhysicalKey) -> bool {
        for k in self.break_events.iter() {
            if *k == key {
                return true;
            }
        }
        return false;
    }

    pub fn clear_events(&mut self) {
        self.make_events.clear();
        self.break_events.clear();
    }
}

//-------------------------------------------------------------------------
// Utilitiy functions to turn a hash function into a random number generator.
// Results in reproducible random numbers.
//-------------------------------------------------------------------------

fn _hash_rand<T>(seed: u64, value: T) -> u64
where
    T: std::hash::Hash,
{
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    value.hash(&mut hasher);
    hasher.finish()
}

pub fn hash_rand_f64<T>(seed: u64, value: T, start_range: f64, end_range: f64) -> f64
where
    T: std::hash::Hash,
{
    let v = _hash_rand(seed, value);
    let v = v as f64 / u64::MAX as f64;
    start_range + v * (end_range - start_range)
}

pub fn hash_rand_u32<T>(seed: u64, value: T, start_range: u32, end_range: u32) -> u32
where
    T: std::hash::Hash,
{
    let v = _hash_rand(seed, value) as u32;
    if end_range == start_range {
        // normally we are selecting from [start,end), but if that is empty just choose start
        // This is similar to float case where empty range selects start.
        start_range
    } else {
        start_range + v % (end_range - start_range)
    }
}

pub trait HashRand<T> {
    fn hash_rand<V: std::hash::Hash>(self, seed: u64, value: V) -> T;
}

impl HashRand<f64> for Range<f64> {
    fn hash_rand<V: std::hash::Hash>(self, seed: u64, value: V) -> f64 {
        hash_rand_f64(seed, value, self.start, self.end)
    }
}

impl HashRand<u32> for Range<u32> {
    fn hash_rand<V: std::hash::Hash>(self, seed: u64, value: V) -> u32 {
        hash_rand_u32(seed, value, self.start, self.end)
    }
}

impl HashRand<Vec2> for Range<Vec2> {
    fn hash_rand<V: std::hash::Hash>(self, seed: u64, value: V) -> Vec2 {
        let seed2 = _hash_rand(seed, value);
        Vec2::new(
            hash_rand_f64(seed, (seed2, "x"), self.start.x, self.end.x),
            hash_rand_f64(seed, (seed2, "y"), self.start.y, self.end.y),
        )
    }
}
