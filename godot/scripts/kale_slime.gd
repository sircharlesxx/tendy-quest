extends CharacterBody3D

const MODEL_PATH := "res://assets/models/kale_slime.glb"
const FIBER_SCRIPT := preload("res://scripts/fiber_trail.gd")

@export var move_speed: float = 2.4
@export var detection_radius: float = 10.0
@export var trail_interval: float = 1.2
@export var damage: float = 15.0

var patrol_origin: Vector3
var patrol_angle: float = 0.0
var trail_timer: float = 0.0
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

	var target_pos: Vector3
	if player and global_position.distance_to(player.global_position) <= detection_radius:
		target_pos = player.global_position
	else:
		patrol_angle += delta * 0.7
		target_pos = patrol_origin + Vector3(cos(patrol_angle) * 3.5, 0.0, sin(patrol_angle) * 3.5)

	var to_target := target_pos - global_position
	to_target.y = 0.0
	if to_target.length() > 0.2:
		var dir := to_target.normalized()
		velocity.x = dir.x * move_speed
		velocity.z = dir.z * move_speed
		rotation.y = lerp_angle(rotation.y, atan2(dir.x, dir.z), 6.0 * delta)

	move_and_slide()

	trail_timer += delta
	if trail_timer >= trail_interval and is_on_floor():
		trail_timer = 0.0
		_spawn_fiber_trail()

	for i in get_slide_collision_count():
		var col := get_slide_collision(i)
		var collider := col.get_collider()
		if collider is Node3D and collider.is_in_group("player") and collider.has_method("take_veggie_hit"):
			var push: Vector3 = (collider.global_position - global_position).normalized() * 7.0
			push.y = 3.5
			collider.take_veggie_hit(damage, push, self)

func _spawn_fiber_trail() -> void:
	var trail := Area3D.new()
	trail.set_script(FIBER_SCRIPT)

	var col := CollisionShape3D.new()
	var shape := CylinderShape3D.new()
	shape.radius = 0.85
	shape.height = 0.25
	col.shape = shape
	trail.add_child(col)

	var mesh_inst := MeshInstance3D.new()
	var cyl := CylinderMesh.new()
	cyl.top_radius = 0.85
	cyl.bottom_radius = 0.9
	cyl.height = 0.08
	var mat := StandardMaterial3D.new()
	mat.albedo_color = Color(0.1, 0.32, 0.12, 0.85)
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	cyl.material = mat
	mesh_inst.mesh = cyl
	trail.add_child(mesh_inst)

	get_parent().add_child(trail)
	trail.global_position = global_position + Vector3(0.0, 0.05, 0.0)
