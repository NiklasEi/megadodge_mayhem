use crate::{aabb::Aabb2, boundaries::Boundaries, collision};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::Rng;

#[derive(Component, Default)]
pub struct Ball {
    /// When a group's bit is set, then some player has already claimed this
    /// ball.
    claims_mask: u8,
    is_held: bool,
}

impl Ball {
    const HELD_OFFSET: Vec3 = Vec3::new(0.4, 0.0, 0.0);

    pub fn is_held(&self) -> bool {
        self.is_held
    }

    pub fn ground_groups() -> CollisionGroups {
        CollisionGroups::new(
            collision::groups::GROUND_BALL,
            collision::groups::QUERY | collision::groups::BOUNDARIES,
        )
    }

    fn thrown_groups() -> CollisionGroups {
        CollisionGroups::new(
            collision::groups::THROWN_BALL,
            collision::groups::THROWN_BALL
                | collision::groups::PLAYER
                | collision::groups::BOUNDARIES,
        )
    }

    pub fn spawn_on_ground(
        commands: &mut Commands,
        ball_assets: &BallAssets,
        bounds: &Boundaries,
        mut position: Vec3,
    ) {
        position.y = ball_assets.radius;
        commands.spawn((
            Self::default(),
            PbrBundle {
                mesh: ball_assets.mesh.clone(),
                material: ball_assets.material.clone(),
                transform: Transform::from_translation(position.clamp(bounds.min, bounds.max)),
                ..default()
            },
            RigidBody::KinematicPositionBased,
            Collider::ball(ball_assets.radius),
            Self::ground_groups(),
        ));
    }

    pub fn spawn_multiple_in_aabb(
        commands: &mut Commands,
        ball_assets: &BallAssets,
        bounds: &Boundaries,
        aabb: Aabb2,
        n_balls: usize,
    ) {
        let mut rng = rand::thread_rng();
        for _ in 0..n_balls {
            let x = rng.gen_range(aabb.min.x..aabb.max.x);
            let z = rng.gen_range(aabb.min.y..aabb.max.y);
            Self::spawn_on_ground(commands, ball_assets, bounds, Vec3::new(x, 0.0, z));
        }
    }

    pub fn pick_up(
        &mut self,
        transform: &mut Transform,
        body: &mut RigidBody,
        groups: &mut CollisionGroups,
    ) {
        self.clear_claims();
        self.is_held = true;
        // TODO: this is going to cause flickering because the transform will be
        // applied before the player can become the parent
        transform.translation = Self::HELD_OFFSET;
        *body = RigidBody::KinematicPositionBased;
        // Don't appear in any collisions or queries until the ball is thrown.
        groups.filters = Group::NONE;
    }

    /// Returns `true` if the claim was successful.
    pub fn claim(&mut self, mask: u8) -> bool {
        let already_claimed = self.is_claimed(mask);
        self.claims_mask |= mask;
        !already_claimed
    }

    pub fn drop_claim(&mut self, mask: u8) {
        self.claims_mask &= !mask;
    }

    pub fn is_claimed(&self, mask: u8) -> bool {
        (self.claims_mask & mask) != 0
    }

    pub fn clear_claims(&mut self) {
        self.claims_mask = 0;
    }

    pub fn spawn_thrown(
        commands: &mut Commands,
        ball_assets: &BallAssets,
        position: Vec3,
        velocity: Vec3,
    ) {
        commands.spawn((
            Self::default(),
            PbrBundle {
                mesh: ball_assets.mesh.clone(),
                material: ball_assets.material.clone(),
                transform: Transform::from_translation(position),
                ..default()
            },
            RigidBody::Dynamic,
            Collider::ball(ball_assets.radius),
            Self::thrown_groups(),
            Ccd::enabled(),
            ActiveEvents::COLLISION_EVENTS,
            // Increased density for better impact.
            ColliderMassProperties::Density(10.0),
            Velocity::linear(velocity),
            Damping {
                // Air friction.
                linear_damping: 0.8,
                angular_damping: 1.0,
            },
            Restitution {
                // Bouncy
                coefficient: 0.9,
                ..default()
            },
            Friction {
                coefficient: 0.7,
                ..default()
            },
        ));
    }
}

#[derive(Resource)]
pub struct BallAssets {
    pub radius: f32,
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

impl BallAssets {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let radius = 0.2;
        Self {
            radius,
            mesh: meshes.add(
                shape::Icosphere {
                    radius,
                    subdivisions: 5,
                }
                .try_into()
                .unwrap(),
            ),
            material: materials.add(Color::RED.into()),
        }
    }
}

#[derive(Component)]
pub struct Cooldown {
    pub timer: Timer,
}
