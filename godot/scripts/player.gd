extends CharacterBody3D

signal stats_changed(tendies: int, craving: float)

const HERO_MODEL_PATH := "res://assets/models/plaid_hero.glb"

@export var move_speed: float = 7.5
@export var dash_speed: float = 16.0
@export var jump_velocity: float = 7.5
@export var rotation_speed: float = 12.0
@export var max_jumps: int = 2
@export var coyote_time: float = 0.12

var gravity: float = ProjectSettings.get_setting("physics/3d/default_gravity")
var craving_meter: float = 100.0
var tendies_collected: int = 0
var dash_timer: float = 0.0
var invuln_timer: float = 0.0
var speed_boost_timer: float = 0.0
var slow_timer: float = 0.0
var has_ranch_shield: bool = false
var coyote_timer: float = 0.0
var jumps_used: int = 0
var spawn_position: Vector3

func _ready() -> void:
	spawn_position = global_position
	_load_model_if_available()
	stats_changed.emit(tendies_collected, craving_meter)

func _load_model_if_available() -> void:
	if ResourceLoader.exists(HERO_MODEL_PATH):
		var packed := load(HERO_MODEL_PATH) as PackedScene
		if packed:
			var model := packed.instantiate()
			add_child(model)
			var fallback := get_node_or_null("FallbackMesh")
			if fallback:
				fallback.visible = false

func _physics_process(delta: float) -> void:
	if is_on_floor():
		coyote_timer = coyote_time
		jumps_used = 0
	else:
		velocity.y -= gravity * delta
		coyote_timer = max(0.0, coyote_timer - delta)

	if Input.is_action_just_pressed("ui_accept"):
		if coyote_timer > 0.0 or jumps_used < max_jumps:
			velocity.y = jump_velocity
			jumps_used = 1 if coyote_timer > 0.0 else jumps_used + 1
			coyote_timer = 0.0

	if Input.is_action_just_pressed("dash") and dash_timer <= 0.0:
		dash_timer = 0.22
		invuln_timer = 0.35

	if dash_timer > 0.0:
		dash_timer -= delta
	if invuln_timer > 0.0:
		invuln_timer -= delta
	if speed_boost_timer > 0.0:
		speed_boost_timer -= delta
	if slow_timer > 0.0:
		slow_timer -= delta

	var input_dir := Input.get_vector("ui_left", "ui_right", "ui_up", "ui_down")
	var direction := Vector3(input_dir.x, 0.0, input_dir.y).normalized()
	var speed_mult := 2.0 if speed_boost_timer > 0.0 else 1.0
	if slow_timer > 0.0:
		speed_mult *= 0.6
	var current_speed := dash_speed if dash_timer > 0.0 else move_speed * speed_mult

	if direction:
		velocity.x = direction.x * current_speed
		velocity.z = direction.z * current_speed
		var target_angle := atan2(direction.x, direction.z)
		rotation.y = lerp_angle(rotation.y, target_angle, rotation_speed * delta)
	else:
		velocity.x = move_toward(velocity.x, 0.0, current_speed)
		velocity.z = move_toward(velocity.z, 0.0, current_speed)

	move_and_slide()

	if global_position.y < -10.0:
		global_position = spawn_position
		velocity = Vector3.ZERO

func collect_tendy(points: int = 100) -> void:
	tendies_collected += 1
	craving_meter = min(100.0, craving_meter + 15.0)
	stats_changed.emit(tendies_collected, craving_meter)
	print("Tendy collected! (+%d pts) Total: %d | Craving: %.0f%%" % [points, tendies_collected, craving_meter])

func activate_honey_mustard(duration: float = 10.0) -> void:
	speed_boost_timer = duration
	print("Honey Mustard Boost! 2x speed for %.0fs" % duration)

func activate_ranch_shield() -> void:
	has_ranch_shield = true
	print("Ranch Shield active! Next veggie hit will be absorbed.")

func apply_fiber_slow(duration: float = 0.5) -> void:
	slow_timer = max(slow_timer, duration)

func take_veggie_hit(damage: float, knockback: Vector3, attacker: Node3D = null) -> void:
	if invuln_timer > 0.0:
		return
	if has_ranch_shield:
		has_ranch_shield = false
		invuln_timer = 0.75
		if attacker and attacker is CharacterBody3D:
			var repel := (attacker.global_position - global_position).normalized() * 12.0
			repel.y = 4.0
			attacker.velocity += repel
		print("Ranch Shield absorbed the hit and knocked the veggie back!")
		return
	invuln_timer = 1.0
	craving_meter = max(0.0, craving_meter - damage)
	velocity += knockback
	stats_changed.emit(tendies_collected, craving_meter)
	print("Ouch! Hit by healthy food! Craving left: %.0f%%" % craving_meter)
