extends CharacterBody3D

enum State { PATROL, CHASE }

const MODEL_PATH := "res://assets/models/broccoli_brute.glb"

@export var patrol_speed: float = 2.5
@export var chase_speed: float = 4.6
@export var detection_radius: float = 9.0
@export var damage: float = 20.0

var state: State = State.PATROL
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

	if player and global_position.distance_to(player.global_position) <= detection_radius:
		state = State.CHASE
	else:
		state = State.PATROL

	var target_pos: Vector3
	var speed: float
	if state == State.CHASE and player:
		target_pos = player.global_position
		speed = chase_speed
	else:
		patrol_angle += delta * 0.9
		target_pos = patrol_origin + Vector3(cos(patrol_angle) * 3.5, 0.0, sin(patrol_angle) * 3.5)
		speed = patrol_speed

	var to_target := target_pos - global_position
	to_target.y = 0.0
	if to_target.length() > 0.15:
		var dir := to_target.normalized()
		velocity.x = dir.x * speed
		velocity.z = dir.z * speed
		rotation.y = lerp_angle(rotation.y, atan2(dir.x, dir.z), 8.0 * delta)

	move_and_slide()

	for i in get_slide_collision_count():
		var col := get_slide_collision(i)
		var collider := col.get_collider()
		if collider is Node3D and collider.is_in_group("player") and collider.has_method("take_veggie_hit"):
			var push: Vector3 = (collider.global_position - global_position).normalized() * 8.0
			push.y = 4.0
			collider.take_veggie_hit(damage, push, self)
