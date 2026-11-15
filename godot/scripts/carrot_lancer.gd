extends CharacterBody3D

enum State { PATROL, TELEGRAPH, LUNGE, STUNNED }

const MODEL_PATH := "res://assets/models/carrot_lancer.glb"

@export var patrol_speed: float = 3.0
@export var lunge_speed: float = 13.5
@export var detection_radius: float = 11.0
@export var damage: float = 20.0

var state: State = State.PATROL
var state_timer: float = 0.0
var lunge_dir: Vector3 = Vector3.FORWARD
var patrol_origin: Vector3
var patrol_angle: float = 0.0
var player: Node3D = null
var gravity: float = ProjectSettings.get_setting("physics/3d/default_gravity")

func _ready() -> void:
	add_to_group("enemies")
	patrol_origin = global_position
	player = get_tree().get_first_node_in_group("player")
	if ResourceLoader.exists(MODEL_PATH):
		var packed := load(MODEL_PATH) as PackedScene
		if packed:
			add_child(packed.instantiate())
			var fallback := get_node_or_null("FallbackMesh")
			if fallback:
				fallback.visible = false

func _physics_process(delta: float) -> void:
	if not is_on_floor():
		velocity.y -= gravity * delta

	match state:
		State.PATROL:
			patrol_angle += delta * 1.1
			var target_pos := patrol_origin + Vector3(cos(patrol_angle) * 4.0, 0.0, sin(patrol_angle) * 4.0)
			var to_target := target_pos - global_position
			to_target.y = 0.0
			if to_target.length() > 0.15:
				var dir := to_target.normalized()
				velocity.x = dir.x * patrol_speed
				velocity.z = dir.z * patrol_speed
				rotation.y = lerp_angle(rotation.y, atan2(dir.x, dir.z), 8.0 * delta)
			if player and global_position.distance_to(player.global_position) <= detection_radius:
				state = State.TELEGRAPH
				state_timer = 0.8
				velocity.x = 0.0
				velocity.z = 0.0

		State.TELEGRAPH:
			state_timer -= delta
			if player:
				var aim := player.global_position - global_position
				aim.y = 0.0
				if aim.length() > 0.1:
					lunge_dir = aim.normalized()
					rotation.y = lerp_angle(rotation.y, atan2(lunge_dir.x, lunge_dir.z), 12.0 * delta)
			if state_timer <= 0.0:
				state = State.LUNGE
				state_timer = 0.65

		State.LUNGE:
			state_timer -= delta
			velocity.x = lunge_dir.x * lunge_speed
			velocity.z = lunge_dir.z * lunge_speed
			if is_on_wall():
				state = State.STUNNED
				state_timer = 1.2
				velocity.x = 0.0
				velocity.z = 0.0
			elif state_timer <= 0.0:
				state = State.PATROL

		State.STUNNED:
			state_timer -= delta
			velocity.x = move_toward(velocity.x, 0.0, 10.0 * delta)
			velocity.z = move_toward(velocity.z, 0.0, 10.0 * delta)
			if state_timer <= 0.0:
				state = State.PATROL

	move_and_slide()

	for i in get_slide_collision_count():
		var col := get_slide_collision(i)
		var collider := col.get_collider()
		if collider is Node3D and collider.is_in_group("player") and collider.has_method("take_veggie_hit"):
			var push: Vector3 = (collider.global_position - global_position).normalized() * 9.0
			push.y = 4.0
			collider.take_veggie_hit(damage, push, self)
