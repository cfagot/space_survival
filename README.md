
# Space Survival

You are in a shipe inside an asteroid field (which is boxed in for some reason). You are running out of air and need to race to pick up the air pod floating amongst the asteroids. Be quick because both your ship and the air pod will lose air over time. After picking up the air pod, a new one will appear some place else in the asteroid field and the race for survival continues. Controls are the arrow keys or W-A-D. Good luck!

Space survival is an example real-time game running inside xilem, rendering using both vello and wgpu. 

Currently there is no xilem gui on top of the game -- that is left for future development. 

The game has no dependencies other than xilem (and xilem's own dependencies), and bytemuck (for wgpu rendering). This is done to show how xilem can be used as a bare-bones game framework out of the box. If one was to develop a larger game with xilem, crates such as hecs would be used instead of the not-really-an-ecs provided by the EntityStore here, and parry would be used for collisions and the spatial database rather than the simplified implementation found here.

Finally, note that this game currently depends the render_hooks branch of my fork of xilem. I'll update to base xilem when possible.
