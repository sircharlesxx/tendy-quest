extends Area3D

const MODEL_PATH := "res://assets/models/golden_tendy.glb"

@export var spin_speed: float = 2.5
@export var bob_amplitude: float = 0.15
var start_y: float = 0.0
var time_passed: float = 0.0

func _ready() -> void:
	start_y = position.y
	body_entered.connect(_on_body_entered)
	if ResourceLoader.exists(MODEL_PATH):
		var packed := load(MODEL_PATH) as PackedScene
		if packed:
			add_child(packed.instantiate())
			var fallback := get_node_or_null("FallbackMesh")
			if fallback:
				fallback.visible = false

func _process(delta: float) -> void:
	time_passed += delta
	rotate_y(spin_speed * delta)
	position.y = start_y + sin(time_passed * 3.5) * bob_amplitude

func _on_body_entered(body: Node3D) -> void:
	if body.is_in_group("player") and body.has_method("collect_tendy"):
		body.collect_tendy(100)
		queue_free()
